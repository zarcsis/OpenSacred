//! The control flow of a NetScript function, structured (#723; docs/re/netscript.md, "Control
//! flow"). The walker runs a function's records one at a time, and its conditional records move
//! its cursor by the records' lengths alone, so that an `IF` chain is flat and an `IF` within a
//! branch shares the chain's alternatives. [`structure`] follows the walker's moves from each
//! record as a graph without cycles, every move going forward, and nests them as tests, branches
//! and returns: the two ways of a test meet again where every way from it passes (its immediate
//! post-dominator), a way that ends the function is written as an early return, and an
//! alternative that both ways reach before they meet is written in each.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use opensacred_data::netscript::{Body, Record, Value};

/// The record types the walker treats apart (docs/re/netscript.md).
const IF: u16 = 58;
const ELSE: u16 = 59;
const ELSEIF: u16 = 66;
const BLOCK: u16 = 41;
const BLOCK_END: u16 = 42;
const EXIT_DLG: u16 = 43;
const DELETE_FUNCTION_BLOCK: u16 = 14;
const TO_BEGIN: u16 = 111;
/// The guards: `QuestDone` to `QuestNotInProgress` (none in the release), `GroupIsDead` and
/// `DelDlg`, whose false result stops the function.
const GUARDS: [u16; 6] = [10, 11, 12, 13, 24, 44];
/// The operand tag by which `DeleteFunktionBlock` names the function running.
const THIS_FUNCTION: u8 = 0x0B;

/// How a function ends: the runner's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ending {
    /// It ran to its end (the runner answers 1).
    Ran,
    /// A record stopped it (the walker answered 0): a guard's false result, `ExitDlg`, a record
    /// of type 0, an `IF` or `ELSEIF` without its `ELSE`, `DeleteFunktionBlock` with names.
    Stopped,
    /// `DeleteFunktionBlock` without names marked it done, never to run again (the walker's 2).
    Done,
    /// `ToBegin` starts it again from its first record (the walker's 3).
    Again,
}

/// What a test tests, by the index of its record in the function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Test {
    /// The condition of an `IF` (58) or an `ELSEIF` (66).
    Condition(usize),
    /// A guard run for its result, whose false result stops the function.
    Guard(usize),
    /// Whether the flag of a block's selector holds (41): its records up to the next 42 run.
    Block(usize),
}

/// A statement of the structured flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// The record of an index run for what it does.
    Run(usize),
    /// A test and the statements of either outcome; an empty `then` reads as a test negated.
    If {
        test: Test,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
    },
    /// The function's end.
    Return(Ending),
}

/// Where the walker stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Node {
    /// At the record of an index, which it runs.
    At(usize),
    /// Reading on from a false condition, at the `ELSEIF` of an index, whose condition it tests.
    ElseIf(usize),
    /// At the function's end.
    End(Ending),
    /// Beyond every end: the root of the post-dominators.
    Sink,
}

/// What the walker does at a node.
enum Step {
    /// Runs the record of an index, if any, and goes on.
    Go(Option<usize>, Node),
    /// Tests and goes to the first node if the test holds, else to the second.
    Branch(Test, Node, Node),
}

/// A function's records and the walker's moves over them.
struct Walker<'a> {
    records: &'a [Record],
}

impl Walker<'_> {
    fn kind(&self, i: usize) -> u16 {
        self.records[i].kind
    }

    /// The walker at the record of index `i`, at the function's end past the last.
    fn at(&self, i: usize) -> Node {
        if i >= self.records.len() {
            Node::End(Ending::Ran)
        } else {
            Node::At(i)
        }
    }

    /// The first record of `kind` from index `from` on.
    fn next(&self, from: usize, kind: u16) -> Option<usize> {
        (from..self.records.len()).find(|&j| self.kind(j) == kind)
    }

    /// Reading on from a false condition, from index `from`: the first `ELSEIF`, whose condition
    /// it tests, or the record after the first `ELSE`; a record of type 0 or the function's end
    /// before either stops the function (`IF mit offenem Ende`).
    fn read_on(&self, from: usize) -> Node {
        for j in from..self.records.len() {
            match self.kind(j) {
                ELSEIF => return Node::ElseIf(j),
                ELSE => return self.at(j + 1),
                0 => return Node::End(Ending::Stopped),
                _ => {}
            }
        }
        Node::End(Ending::Stopped)
    }

    /// Whether `DeleteFunktionBlock` names functions, by strings or by the tag of the function
    /// running.
    fn names_functions(&self, i: usize) -> bool {
        match &self.records[i].body {
            Body::Operands(operands) => operands.iter().any(|o| {
                o.tag == THIS_FUNCTION || matches!(o.value, Value::Str(_) | Value::Strs(..))
            }),
            _ => false,
        }
    }

    fn step(&self, node: Node) -> Option<Step> {
        let i = match node {
            Node::At(i) => i,
            Node::ElseIf(j) => {
                return Some(Step::Branch(
                    Test::Condition(j),
                    self.at(j + 1),
                    self.read_on(j + 1),
                ));
            }
            Node::End(_) | Node::Sink => return None,
        };
        let kind = self.kind(i);
        Some(match kind {
            0 => Step::Go(None, Node::End(Ending::Stopped)),
            EXIT_DLG => Step::Go(Some(i), Node::End(Ending::Stopped)),
            IF => Step::Branch(Test::Condition(i), self.at(i + 1), self.read_on(i + 1)),
            // Reached in the flow, its branch having run: past the record after the next `ELSE`.
            ELSEIF => Step::Go(
                None,
                self.next(i + 1, ELSE)
                    .map_or(Node::End(Ending::Stopped), |e| self.at(e + 2)),
            ),
            ELSE => Step::Go(None, self.at(i + 2)),
            BLOCK => Step::Branch(
                Test::Block(i),
                self.at(i + 1),
                self.next(i + 1, BLOCK_END)
                    .map_or(Node::End(Ending::Ran), |e| self.at(e + 1)),
            ),
            BLOCK_END => Step::Go(None, self.at(i + 1)),
            DELETE_FUNCTION_BLOCK => Step::Go(
                Some(i),
                Node::End(if self.names_functions(i) {
                    Ending::Stopped
                } else {
                    Ending::Done
                }),
            ),
            TO_BEGIN => Step::Go(None, Node::End(Ending::Again)),
            kind if GUARDS.contains(&kind) => {
                Step::Branch(Test::Guard(i), self.at(i + 1), Node::End(Ending::Stopped))
            }
            _ => Step::Go(Some(i), self.at(i + 1)),
        })
    }

    fn successors(&self, node: Node) -> Vec<Node> {
        match (node, self.step(node)) {
            (_, Some(Step::Go(_, next))) => vec![next],
            (_, Some(Step::Branch(_, a, b))) => vec![a, b],
            (Node::End(_), None) => vec![Node::Sink],
            _ => Vec::new(),
        }
    }
}

/// The walker's moves from a function's first record and their immediate post-dominators.
struct Graph<'a> {
    walker: Walker<'a>,
    ipdom: BTreeMap<Node, Node>,
}

impl<'a> Graph<'a> {
    fn new(records: &'a [Record]) -> Self {
        let walker = Walker { records };
        let mut nodes = BTreeSet::new();
        let mut stack = vec![walker.at(0)];
        while let Some(node) = stack.pop() {
            if nodes.insert(node) {
                stack.extend(walker.successors(node));
            }
        }
        // Every move goes forward, to a later record, an end or the sink: the nodes' order by
        // their records is one in which each comes before those it moves to.
        let position = |node: &Node| match *node {
            Node::At(i) | Node::ElseIf(i) => (0, i),
            Node::End(_) => (1, 0),
            Node::Sink => (2, 0),
        };
        let mut order: Vec<Node> = nodes.into_iter().collect();
        order.sort_by_key(|node| (position(node), *node));
        let rank: BTreeMap<Node, usize> = order.iter().enumerate().map(|(r, &n)| (n, r)).collect();
        let mut ipdom = BTreeMap::new();
        for &node in order.iter().rev() {
            let mut successors = walker.successors(node).into_iter();
            let Some(first) = successors.next() else {
                continue;
            };
            let meet = successors.fold(first, |a, b| meet(&rank, &ipdom, a, b));
            ipdom.insert(node, meet);
        }
        Self { walker, ipdom }
    }

    /// The statements from `from` until `until`, at most `budget` more of them.
    fn emit(&self, from: Node, until: Node, budget: &mut usize) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        let mut node = from;
        while node != until {
            if *budget == 0 {
                bail!("its flow does not structure within its budget");
            }
            *budget -= 1;
            match node {
                Node::End(ending) => {
                    out.push(Stmt::Return(ending));
                    break;
                }
                Node::Sink => break,
                _ => {}
            }
            match self.walker.step(node) {
                Some(Step::Go(run, next)) => {
                    out.extend(run.map(Stmt::Run));
                    node = next;
                }
                Some(Step::Branch(test, yes, no)) => {
                    // A way that ends the function at once is an early return.
                    if let Node::End(ending) = no
                        && yes != no
                    {
                        out.push(Stmt::If {
                            test,
                            then: Vec::new(),
                            otherwise: vec![Stmt::Return(ending)],
                        });
                        node = yes;
                        continue;
                    }
                    if let Node::End(ending) = yes
                        && yes != no
                    {
                        out.push(Stmt::If {
                            test,
                            then: vec![Stmt::Return(ending)],
                            otherwise: Vec::new(),
                        });
                        node = no;
                        continue;
                    }
                    let join = self.ipdom.get(&node).copied().unwrap_or(Node::Sink);
                    let then = self.emit(yes, join, budget)?;
                    let otherwise = self.emit(no, join, budget)?;
                    out.push(Stmt::If {
                        test,
                        then,
                        otherwise,
                    });
                    node = join;
                }
                None => bail!("the walker has no move at {node:?}"),
            }
        }
        Ok(out)
    }
}

/// Where the post-dominator chains of `a` and `b` meet: their nearest common post-dominator.
fn meet(rank: &BTreeMap<Node, usize>, ipdom: &BTreeMap<Node, Node>, a: Node, b: Node) -> Node {
    let (mut a, mut b) = (a, b);
    while a != b {
        if rank[&a] < rank[&b] {
            a = ipdom.get(&a).copied().unwrap_or(Node::Sink);
        } else {
            b = ipdom.get(&b).copied().unwrap_or(Node::Sink);
        }
    }
    a
}

/// The structured flow of a function of `records`, as the walker runs it; an error where the
/// statements it takes, with the alternatives written in each way that reaches them, pass
/// fifty for each record and a hundred more.
pub fn structure(records: &[Record]) -> Result<Vec<Stmt>> {
    let graph = Graph::new(records);
    let mut budget = records.len() * 50 + 100;
    graph.emit(graph.walker.at(0), Node::Sink, &mut budget)
}

#[cfg(test)]
pub(crate) mod tests {
    use opensacred_data::netscript::Operand;

    use super::*;

    /// The tests' dice, a xorshift.
    pub(crate) struct Dice(u64);

    impl Dice {
        pub(crate) fn new(seed: u32) -> Self {
            Self(u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }

        pub(crate) fn below(&mut self, n: u32) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            u32::try_from(self.0 % u64::from(n)).unwrap_or(0)
        }
    }

    /// What the walker or the structured flow did, in order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum Event {
        Run(usize),
        Test(Test, bool),
        End(Ending),
    }

    /// The walker itself, record by record, as docs/re/netscript.md tells it, with the outcomes
    /// of its tests from `outcome`: the reference that [`structure`] must follow.
    pub(crate) fn walk(records: &[Record], outcome: &mut impl FnMut() -> bool) -> Vec<Event> {
        let mut events = Vec::new();
        let n = records.len();
        let kind = |i: usize| records[i].kind;
        let mut i = 0;
        let mut skipping = false;
        loop {
            if i >= n {
                events.push(Event::End(Ending::Ran));
                return events;
            }
            if skipping {
                if kind(i) == BLOCK_END {
                    skipping = false;
                }
                i += 1;
                continue;
            }
            let mut test = |test: Test, events: &mut Vec<Event>| {
                let holds = outcome();
                events.push(Event::Test(test, holds));
                holds
            };
            match kind(i) {
                0 => {
                    events.push(Event::End(Ending::Stopped));
                    return events;
                }
                EXIT_DLG => {
                    events.push(Event::Run(i));
                    events.push(Event::End(Ending::Stopped));
                    return events;
                }
                IF => {
                    let holds = test(Test::Condition(i), &mut events);
                    i += 1;
                    if !holds {
                        // Read on to an `ELSEIF` that holds or an `ELSE`.
                        loop {
                            if i >= n || kind(i) == 0 {
                                events.push(Event::End(Ending::Stopped));
                                return events;
                            }
                            if kind(i) == ELSEIF {
                                let holds = test(Test::Condition(i), &mut events);
                                i += 1;
                                if holds {
                                    break;
                                }
                                continue;
                            }
                            if kind(i) == ELSE {
                                i += 1;
                                break;
                            }
                            i += 1;
                        }
                    }
                }
                ELSEIF => match (i + 1..n).find(|&j| kind(j) == ELSE) {
                    Some(e) => i = e + 2,
                    None => {
                        events.push(Event::End(Ending::Stopped));
                        return events;
                    }
                },
                ELSE => i += 2,
                BLOCK => {
                    skipping = !test(Test::Block(i), &mut events);
                    i += 1;
                }
                BLOCK_END => i += 1,
                DELETE_FUNCTION_BLOCK => {
                    events.push(Event::Run(i));
                    let named = Walker { records }.names_functions(i);
                    events.push(Event::End(if named {
                        Ending::Stopped
                    } else {
                        Ending::Done
                    }));
                    return events;
                }
                TO_BEGIN => {
                    events.push(Event::End(Ending::Again));
                    return events;
                }
                k if GUARDS.contains(&k) => {
                    if !test(Test::Guard(i), &mut events) {
                        events.push(Event::End(Ending::Stopped));
                        return events;
                    }
                    i += 1;
                }
                _ => {
                    events.push(Event::Run(i));
                    i += 1;
                }
            }
        }
    }

    /// The structured flow run with the outcomes of its tests from `outcome`.
    fn run(stmts: &[Stmt], outcome: &mut impl FnMut() -> bool, events: &mut Vec<Event>) -> bool {
        for stmt in stmts {
            match stmt {
                Stmt::Run(i) => events.push(Event::Run(*i)),
                Stmt::Return(ending) => {
                    events.push(Event::End(*ending));
                    return true;
                }
                Stmt::If {
                    test,
                    then,
                    otherwise,
                } => {
                    let holds = outcome();
                    events.push(Event::Test(*test, holds));
                    if run(if holds { then } else { otherwise }, outcome, events) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn records(kinds: &[u16]) -> Vec<Record> {
        kinds
            .iter()
            .map(|&kind| Record {
                kind,
                body: Body::Operands(Vec::new()),
            })
            .collect()
    }

    /// Whether the structured flow of `records` makes the walker's events for `tries` sets of
    /// outcomes of the dice of `seed`.
    fn follows(records: &[Record], seed: u32, tries: u32) -> Result<(), String> {
        let stmts = structure(records).map_err(|e| e.to_string())?;
        let mut dice = Dice::new(seed);
        for _ in 0..tries {
            let outcomes: Vec<bool> = (0..64).map(|_| dice.below(2) == 1).collect();
            let mut a = outcomes.iter().copied().cycle();
            let mut b = outcomes.iter().copied().cycle();
            let walked = walk(records, &mut || a.next().unwrap_or(false));
            let mut ran = Vec::new();
            run(&stmts, &mut || b.next().unwrap_or(false), &mut ran);
            if walked != ran {
                return Err(format!("walked {walked:?}, ran {ran:?}, from {stmts:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn an_if_chain_runs_one_branch_and_an_else_one_record() {
        // IF a; A; ELSEIF b; B; ELSE; x; rest.
        let chain = records(&[IF, 1, ELSEIF, 2, ELSE, 3, 4]);
        let stmts = structure(&chain).unwrap();
        assert_eq!(
            stmts,
            [
                Stmt::If {
                    test: Test::Condition(0),
                    then: vec![Stmt::Run(1)],
                    otherwise: vec![Stmt::If {
                        test: Test::Condition(2),
                        then: vec![Stmt::Run(3)],
                        otherwise: vec![Stmt::Run(5)],
                    }],
                },
                Stmt::Run(6),
                Stmt::Return(Ending::Ran),
            ]
        );
        follows(&chain, 1, 32).unwrap();
    }

    #[test]
    fn the_censuss_forms_run_as_the_walker_runs_them() {
        for (form, kinds) in [
            // An `IF` in the last branch shares the chain's `ELSE`.
            (
                "if in the last branch",
                &[IF, 1, ELSEIF, 2, IF, 3, ELSE, 4, 5][..],
            ),
            // An `IF` after the `ELSE`.
            ("else if", &[IF, 1, ELSE, IF, 2, ELSE, 3, 4][..]),
            // Two `ELSE`s.
            ("else else", &[IF, 1, ELSE, ELSE, 2, 3][..]),
            // An `IF` at the function's end, before one record: a guard.
            ("open if", &[1, IF, 2][..]),
            // An `ELSEIF` without an `ELSE`.
            ("open elseif", &[IF, 1, ELSEIF, 2, 3][..]),
            // Blocks, guards and the ends.
            (
                "blocks",
                &[BLOCK, 1, IF, 2, ELSE, 3, BLOCK_END, BLOCK, 4, BLOCK_END, 5][..],
            ),
            ("guards", &[24, 1, 44, 2, EXIT_DLG, 3][..]),
            ("again", &[1, IF, TO_BEGIN, ELSE, 2, 3][..]),
            ("done", &[1, IF, DELETE_FUNCTION_BLOCK, ELSE, 2][..]),
            ("type 0", &[IF, 1, ELSE, 0, 2][..]),
            (
                "nested",
                &[IF, IF, 1, ELSEIF, 2, ELSE, 3, ELSEIF, 4, ELSE, 5, 6][..],
            ),
        ] {
            follows(&records(kinds), 3, 64).unwrap_or_else(|e| panic!("{form}: {e}"));
        }
    }

    #[test]
    fn any_records_run_as_the_walker_runs_them() {
        let alphabet = [
            1, 2, IF, IF, ELSEIF, ELSE, ELSE, BLOCK, BLOCK_END, 24, EXIT_DLG, 0,
        ];
        let mut dice = Dice::new(11);
        for seed in 0..400 {
            let n = dice.below(24) + 1;
            let kinds: Vec<u16> = (0..n)
                .map(|_| alphabet[usize::try_from(dice.below(12)).unwrap_or(0)])
                .collect();
            follows(&records(&kinds), seed, 16).unwrap_or_else(|e| panic!("{kinds:?}: {e}"));
        }
    }

    /// Every function and script of the 20 trees structures, 403,900 of them (docs/02 §7.4), and
    /// its flow runs as the walker runs it for eight sets of outcomes each.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_function_of_the_trees_runs_as_the_walker_runs_it() {
        let game = crate::testing::repo_root().join("OriginalGame");
        let mut functions = 0;
        for source in crate::netscript::trees().unwrap() {
            let tree = crate::netscript::tree(&game, &source).unwrap();
            let scripts = [("StartCode", &tree.start), ("QuestCode", &tree.quest)];
            let named = tree.functions.iter().map(|f| (f.name.as_str(), &f.records));
            for (i, (name, records)) in scripts.into_iter().chain(named).enumerate() {
                let seed = u32::try_from(i).unwrap_or(0);
                follows(records, seed, 8).unwrap_or_else(|e| panic!("{source} {name}: {e}"));
                functions += 1;
            }
        }
        assert_eq!(functions, 403_900);
    }

    #[test]
    fn delete_function_block_ends_by_whether_it_names_functions() {
        let named = |operands: Vec<Operand>| {
            vec![Record {
                kind: DELETE_FUNCTION_BLOCK,
                body: Body::Operands(operands),
            }]
        };
        let ending = |records: &[Record]| structure(records).unwrap().last().cloned();
        assert_eq!(ending(&named(Vec::new())), Some(Stmt::Return(Ending::Done)));
        assert_eq!(
            ending(&named(vec![Operand {
                tag: THIS_FUNCTION,
                value: Value::Marker,
            }])),
            Some(Stmt::Return(Ending::Stopped))
        );
        assert_eq!(
            ending(&named(vec![Operand {
                tag: 0x01,
                value: Value::Str("OtherFunction".into()),
            }])),
            Some(Stmt::Return(Ending::Stopped))
        );
    }
}
