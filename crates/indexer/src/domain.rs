use std::collections::HashMap;

use anyhow::anyhow;
use parser_core::instruction::{InstructionParser, Parsed, Resource};
use solana_sdk::pubkey::Pubkey;
use types::{Instruction, RawTx};

use crate::{Indexer, IndexerBuilder, ProcessedTx, build_processed, interpreter::Event};

/// A cohesive set of programs and how to interpret them.
/// What makes something "a domain" is that it owns it sets of parsers,
/// its payload type, and its own way of turning a parsed transaction into events.
///
/// A `program_id` must be claimed by only one [Domain].
pub trait Domain {
    /// The [Domain] interpretation of an instruction (ie: [Operation] for a trade, a future
    /// `GameMove` for a game domain, etc).
    type Payload: Resource + Default + Copy;
    /// This [Domain]'s emitted [Event] type.
    type Event: Event + 'static;

    /// Parsers this [Domain] owns, keyed by the `program_id` they interpret.
    /// This is also the source of truth for which `program_id` this [Domain] claims.
    fn parsers(&self) -> HashMap<Pubkey, Box<dyn InstructionParser<Self::Payload> + Send + Sync>>;

    /// Runs this [Domain]'s interpretation pipeline over one transaction's already parsed
    /// instructions, and returns whatever events it found.
    /// `processed` must be built from an instruction list where only the instructions this [Domain]
    /// own were parsed by its own [Domain::parsers]. Every other instruction some other domain owns
    /// show up here as `Self::Payload::default()`.
    fn events(&self, tx: &RawTx, processed: &ProcessedTx<Self::Payload>) -> Vec<Self::Event>;
}

/// A [Domain] is not object-safe (it contains references to `Self::Payload`), but we'll need the
/// driver to hold several [Domain]s and treat them the same way.
/// So [RunDomain] is an object-safe wrapper for it.
pub trait RunDomain {
    /// Program IDs this domain claims.
    fn program_ids(&self) -> Vec<Pubkey>;

    /// Parse or push a default placeholder (if not) for one
    /// instruction, in tx order. Called once per instruction, for every
    /// registered domain, by the driver's single walk.
    fn push(&mut self, instr: &Instruction) -> Result<(), anyhow::Error>;

    /// After every instruction has been pushed, build this domain's
    /// [CallTree]/[ExecutionGraph] and run events().
    /// This drains whatever instructions have accumulated so far with [RunDomain::push].
    fn flush(&mut self, tx: &RawTx) -> Result<Vec<Box<dyn Event>>, anyhow::Error>;
}

/// Runs one [Domain]'s parse -> interpret -> event pipeline. Built once per
/// registered domain, then driven one instruction at a time by the shared
/// walk in the driver (via [RunDomain]) rather than walking the transaction
/// itself
/// That way, we walk the transaction's instructions once overall.
///
/// Wraps an `Indexer<D::Payload>` to reuse its parser lookup and
/// `CallTree`/`ExecutionGraph` construction (`Indexer::parse_one`,
/// `build_processed`) rather than re-deriving that logic here - see
/// `Indexer`'s own docs for what each piece does.
pub struct DomainRunner<D: Domain> {
    /// The [Domain] being run, which owns the event-building logic ([Domain::events]).
    domain: D,
    /// The [Domain]'s own parsers, used to resolve the `program_id` -> parser and parse
    /// a single instruction ([Indexer::parse_one]).
    indexer: Indexer<D::Payload>,
    /// The [Domain] ordered parsed instructions.
    /// It follows the same logic as [Indexer::process], but accumulated one instruction
    /// at a time (from outside) across repeated [RunDomain::push] calls.
    parsed: Vec<Parsed<D::Payload>>,
}

impl<D: Domain> DomainRunner<D> {
    fn new(domain: D) -> Self {
        let mut indexer_builder = IndexerBuilder::new();
        for (program_id, parser) in domain.parsers().into_iter() {
            indexer_builder = indexer_builder.with_boxed_parser(program_id, parser);
        }
        DomainRunner {
            domain,
            indexer: indexer_builder.build(),
            parsed: Vec::new(),
        }
    }
}

impl<D: Domain> RunDomain for DomainRunner<D> {
    fn program_ids(&self) -> Vec<Pubkey> {
        self.indexer.parsers.keys().copied().collect()
    }

    fn push(&mut self, instr: &Instruction) -> Result<(), anyhow::Error> {
        let parsed = self.indexer.parse_one(instr)?;
        self.parsed.push(parsed);
        Ok(())
    }

    fn flush(&mut self, tx: &RawTx) -> Result<Vec<Box<dyn Event>>, anyhow::Error> {
        let processed = build_processed(std::mem::take(&mut self.parsed));
        Ok(self
            .domain
            .events(tx, &processed)
            .into_iter()
            .map(|e| Box::new(e) as Box<dyn Event>)
            .collect())
    }
}

/// Holds every registered [Domain] (type-erased via [RunDomain]) and drives
/// the single shared walk over a transaction's instructions that all of
/// them participate in (see [DomainRegistry::process]).
pub struct DomainRegistry {
    domains: Vec<Box<dyn RunDomain>>,
}

impl DomainRegistry {
    /// Builds the registry, enforcing that each program is handled by only one domain invariant
    /// up front: fails loudly if two domains' [RunDomain::program_ids]
    /// claim the same `program_id`, rather than letting one silently
    /// shadow the other later, during [DomainRegistry::process].
    pub fn new(domains: Vec<Box<dyn RunDomain>>) -> Result<Self, anyhow::Error> {
        let mut programs = HashMap::new();
        for (idx, domain) in domains.iter().enumerate() {
            for program_id in domain.program_ids() {
                if programs.insert(program_id, idx).is_some() {
                    return Err(anyhow!(
                        "Program id {} is already owned by domain idx {}",
                        program_id,
                        idx
                    ));
                }
            }
        }
        Ok(DomainRegistry { domains })
    }

    /// Runs every registered domain over one transaction in a single pass
    /// over `tx.instructions`: each instruction is offered to every domain
    /// via [RunDomain::push] (each domain parses it if it owns the
    /// `program_id`, defaults otherwise), then every domain is
    /// [RunDomain::flush]ed to collect whatever events it found.
    /// The registry (and each domain inside it) is reusable across calls:
    /// `flush` only drains the per-transaction buffer, not the domain or
    /// its parsers.
    pub fn process(&mut self, tx: &RawTx) -> Result<Vec<Box<dyn Event>>, anyhow::Error> {
        let mut events = Vec::new();
        if let Some(err) = &tx.status {
            return Err(anyhow!("Transaction failed: {:}", err));
        }

        for instr in &tx.instructions {
            for domain in &mut self.domains {
                domain.push(instr)?;
            }
        }

        for domain in &mut self.domains {
            events.extend(domain.flush(tx)?);
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use anyhow::anyhow;
    use proptest::{prelude::*, prop_assert, prop_assert_eq};
    use solana_sdk::{signature::Signature, transaction::TransactionError};
    use test_strategy::proptest;
    use types::{Instruction, RawTx};

    use super::*;

    /// A small, deterministic set of distinct program ids, big enough to
    /// exercise overlap/no-overlap combinations, small enough to keep a
    /// shrunk proptest failure readable.
    fn pool(n: usize) -> Vec<Pubkey> {
        (0..n)
            .map(|i| Pubkey::new_from_array([i as u8; 32]))
            .collect()
    }

    fn instruction(program_id: Pubkey) -> Instruction {
        Instruction {
            program_id,
            accounts: vec![],
            stack_depth: 0,
            outer_idx: 0,
            data: vec![],
        }
    }

    fn raw_tx(instructions: Vec<Instruction>, status: Option<TransactionError>) -> RawTx {
        RawTx {
            instructions,
            status,
            ..Default::default()
        }
    }

    /// A minimal, hand-rolled [RunDomain] - deliberately not a real
    /// [Domain] (no payload, no parsers, no `CallTree`/`ExecutionGraph`),
    /// since these tests are about [DomainRegistry]'s own orchestration
    /// (dispatch, fail-fast, error propagation, reuse) and not about
    /// anything a real domain implementation does. Records every
    /// `program_id` it's ever been `push`ed, behind an `Rc<RefCell<_>>` so
    /// a test can keep its own handle after the domain is boxed and moved
    /// into a registry.
    struct TestDomain {
        id: u8,
        claims: Vec<Pubkey>,
        seen: Rc<RefCell<Vec<Pubkey>>>,
        /// If `Some(n)`, `push` starts failing once `n` instructions have
        /// already been recorded - lets a test force a mid-walk error.
        fail_after: Option<usize>,
    }

    impl TestDomain {
        fn new(id: u8, claims: Vec<Pubkey>) -> Self {
            TestDomain {
                id,
                claims,
                seen: Rc::new(RefCell::new(Vec::new())),
                fail_after: None,
            }
        }
    }

    struct TestEvent {
        domain: u8,
        program_id: Pubkey,
    }

    impl Event for TestEvent {
        fn program_id(&self) -> Pubkey {
            self.program_id
        }

        fn describe(&self) -> String {
            format!("domain {} saw {}", self.domain, self.program_id)
        }
    }

    impl RunDomain for TestDomain {
        fn program_ids(&self) -> Vec<Pubkey> {
            self.claims.clone()
        }

        fn push(&mut self, instr: &Instruction) -> Result<(), anyhow::Error> {
            let mut seen = self.seen.borrow_mut();
            if let Some(n) = self.fail_after {
                if seen.len() >= n {
                    return Err(anyhow!("domain {} refuses push #{}", self.id, seen.len()));
                }
            }
            seen.push(instr.program_id);
            Ok(())
        }

        fn flush(&mut self, _tx: &RawTx) -> Result<Vec<Box<dyn Event>>, anyhow::Error> {
            Ok(self
                .seen
                .borrow_mut()
                .drain(..)
                .map(|program_id| {
                    Box::new(TestEvent {
                        domain: self.id,
                        program_id,
                    }) as Box<dyn Event>
                })
                .collect())
        }
    }

    fn boxed(domain: TestDomain) -> Box<dyn RunDomain> {
        Box::new(domain)
    }

    /// Every domain gets an independent random subset of a small shared
    /// pool of program ids - deliberately *not* guaranteed disjoint, so
    /// overlaps (the thing [DomainRegistry::new] must reject) happen with
    /// real probability instead of needing to be forced.
    fn claim_matrix() -> impl Strategy<Value = Vec<Vec<bool>>> {
        (2..=4usize, 1..=6usize).prop_flat_map(|(num_domains, pool_size)| {
            prop::collection::vec(prop::collection::vec(any::<bool>(), pool_size), num_domains)
        })
    }

    /// `DomainRegistry::new` must succeed exactly when no program id in the
    /// shared pool is claimed by more than one domain - the
    /// one-program-one-domain invariant, checked independently of the
    /// registry's own logic (by directly counting overlaps in the
    /// generated matrix), so this can't pass by coincidentally agreeing
    /// with a buggy implementation.
    #[proptest]
    fn new_fails_iff_two_domains_claim_the_same_program_id(
        #[strategy(claim_matrix())] matrix: Vec<Vec<bool>>,
    ) {
        let pool_size = matrix[0].len();
        let addrs = pool(pool_size);
        let has_conflict =
            (0..pool_size).any(|slot| matrix.iter().filter(|row| row[slot]).count() >= 2);

        let domains: Vec<Box<dyn RunDomain>> = matrix
            .iter()
            .enumerate()
            .map(|(id, row)| {
                let claims = row
                    .iter()
                    .enumerate()
                    .filter(|(_, claimed)| **claimed)
                    .map(|(slot, _)| addrs[slot])
                    .collect();
                boxed(TestDomain::new(id as u8, claims))
            })
            .collect();

        prop_assert_eq!(DomainRegistry::new(domains).is_ok(), !has_conflict);
    }

    /// Each program id in the pool is owned by at most one domain (or
    /// none) - unlike [claim_matrix], this always produces a valid
    /// registration, so the test below can focus purely on
    /// [DomainRegistry::process]'s dispatch behavior.
    fn process_scenario() -> impl Strategy<Value = (usize, Vec<Option<usize>>, Vec<usize>)> {
        (1..=4usize, 1..=6usize).prop_flat_map(|(num_domains, pool_size)| {
            (
                Just(num_domains),
                prop::collection::vec(prop::option::of(0..num_domains), pool_size),
                prop::collection::vec(0..pool_size, 0..=15),
            )
        })
    }

    /// Every registered domain must see every instruction, in the
    /// transaction's original order - regardless of whether that domain
    /// "owns" the instruction's program id. Ownership-based filtering is a
    /// `DomainRunner`/parser concern (`Indexer::parse_one`); this test only
    /// pins down what [DomainRegistry] itself is responsible for: the
    /// shared walk reaches everyone, in order, exactly once per
    /// instruction.
    #[proptest]
    fn process_dispatches_every_instruction_to_every_domain_in_order(
        #[strategy(process_scenario())] scenario: (usize, Vec<Option<usize>>, Vec<usize>),
    ) {
        let (num_domains, owners, program_idxs) = scenario;
        let pool_size = owners.len();
        let addrs = pool(pool_size);

        let domains: Vec<Box<dyn RunDomain>> = (0..num_domains)
            .map(|id| {
                let claims = owners
                    .iter()
                    .enumerate()
                    .filter(|(_, owner)| **owner == Some(id))
                    .map(|(slot, _)| addrs[slot])
                    .collect();
                boxed(TestDomain::new(id as u8, claims))
            })
            .collect();
        let mut registry = DomainRegistry::new(domains).unwrap();

        let instructions: Vec<Instruction> = program_idxs
            .iter()
            .map(|&i| instruction(addrs[i]))
            .collect();
        let expected: Vec<Pubkey> = program_idxs.iter().map(|&i| addrs[i]).collect();

        let tx = raw_tx(instructions, None);
        let events = registry.process(&tx).unwrap();

        if expected.is_empty() {
            prop_assert!(events.is_empty());
        } else {
            prop_assert_eq!(events.len(), num_domains * expected.len());
            for chunk in events.chunks(expected.len()) {
                let got: Vec<Pubkey> = chunk.iter().map(|e| e.program_id()).collect();
                prop_assert_eq!(&got, &expected);
            }
        }
    }

    #[test]
    fn process_short_circuits_before_touching_any_domain_on_failed_tx() {
        let addr = pool(1)[0];
        let domain = TestDomain::new(0, vec![addr]);
        let seen = domain.seen.clone();
        let mut registry = DomainRegistry::new(vec![boxed(domain)]).unwrap();

        let tx = raw_tx(
            vec![instruction(addr)],
            Some(TransactionError::AccountInUse),
        );

        assert!(registry.process(&tx).is_err());
        assert!(
            seen.borrow().is_empty(),
            "push should never run once tx.status is set"
        );
    }

    #[test]
    fn process_propagates_a_domains_push_error() {
        let addr = pool(1)[0];
        let mut domain = TestDomain::new(0, vec![addr]);
        domain.fail_after = Some(0);
        let mut registry = DomainRegistry::new(vec![boxed(domain)]).unwrap();

        let tx = raw_tx(vec![instruction(addr)], None);
        assert!(registry.process(&tx).is_err());
    }

    #[test]
    fn process_is_reusable_across_multiple_transactions() {
        let addr = pool(1)[0];
        let domain = TestDomain::new(0, vec![addr]);
        let mut registry = DomainRegistry::new(vec![boxed(domain)]).unwrap();

        let tx1 = raw_tx(vec![instruction(addr), instruction(addr)], None);
        let events1 = registry.process(&tx1).unwrap();
        assert_eq!(events1.len(), 2);

        let tx2 = raw_tx(vec![instruction(addr)], None);
        let events2 = registry.process(&tx2).unwrap();
        assert_eq!(
            events2.len(),
            1,
            "leftover state from the previous transaction must not leak into the next flush"
        );
    }
}
