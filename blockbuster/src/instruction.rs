use plerkle_serialization::{CompiledInstruction, Pubkey, TransactionInfo};

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

pub type IxPair<'a> = (Pubkey, CompiledInstruction<'a>);

pub struct InstructionBundle<'a> {
    pub txn_id: &'a str,
    pub program: Pubkey,
    pub instruction: Option<CompiledInstruction<'a>>,
    pub inner_ix: Option<Vec<IxPair<'a>>>,
    pub keys: &'a [Pubkey],
    pub slot: u64,
}

impl<'a> Default for InstructionBundle<'a> {
    fn default() -> Self {
        InstructionBundle {
            txn_id: "",
            program: Pubkey::new(&[0; 32]),
            instruction: None,
            inner_ix: None,
            keys: &[],
            slot: 0,
        }
    }
}

pub fn order_instructions<'a>(
    programs: HashSet<&[u8]>,
    transaction_info: &'a TransactionInfo<'a>,
) -> VecDeque<(IxPair<'a>, Option<Vec<IxPair<'a>>>)> {
    let mut ordered_ixs: VecDeque<(IxPair, Option<Vec<IxPair>>)> = VecDeque::new();
    // Get outer instructions.
    let outer_instructions = match transaction_info.outer_instructions() {
        None => {
            println!("outer instructions deserialization error");
            return ordered_ixs;
        }
        Some(instructions) => {
            println!(
                "BLOCKBUSTER: total outer instructions: {} for tx: {:?}",
                instructions.len(),
                transaction_info.signature()
            );
            instructions
        }
    };

    if transaction_info.account_keys().is_none() {
        return ordered_ixs;
    }
    // Get account keys.
    let keys = RefCell::new(
        transaction_info
            .account_keys()
            .iter()
            .flatten()
            .collect::<Vec<_>>(),
    );
    println!("Transaction Keys after flatten {:?}", keys);

    // Get inner instructions.
    let legacy_inner_ix_list = transaction_info.inner_instructions();
    let compiled_inner_instructions = transaction_info.compiled_inner_instructions();
    for (outer_instruction_index, outer_instruction) in outer_instructions.iter().enumerate() {
        let non_hoisted_inner_instruction =
            if let Some(inner_instructions) = compiled_inner_instructions {
                inner_instructions
                    .iter()
                    .filter(|x| x.index() == outer_instruction_index as u8)
                    .flat_map(|x| {
                        if let Some(ixes) = x.instructions() {
                            let mut instructions = Vec::<IxPair>::new();
                            ixes.iter()
                                .filter_map(|ix| ix.compiled_instruction())
                                .for_each(|ix| {
                                    let kb = keys.borrow();
                                    println!(
                                    "Transaction Keys len after borrow {} and program_id_index {}",
                                    kb.len(),
                                    ix.program_id_index()
                                );
                                    let program_id = kb.get(ix.program_id_index() as usize);
                                    if let Some(program_id) = program_id {
                                        instructions.push((**program_id, ix))
                                    }
                                });
                            instructions
                        } else {
                            Vec::new()
                        }
                    })
                    .collect::<Vec<IxPair>>()
            } else {
                // legacy no stack height list must exist if no compiled or no processing will be done
                let inner_instructions = legacy_inner_ix_list.unwrap();
                inner_instructions
                    .iter()
                    .filter(|x| x.index() == outer_instruction_index as u8)
                    .flat_map(|x| {
                        if let Some(ixes) = x.instructions() {
                            let mut instructions = Vec::<IxPair>::new();
                            ixes.iter().for_each(|ix| {
                                let kb = keys.borrow();
                                let program_id = kb.get(ix.program_id_index() as usize);
                                if let Some(program_id) = program_id {
                                    instructions.push((**program_id, ix))
                                }
                            });
                            instructions
                        } else {
                            Vec::new()
                        }
                    })
                    .collect::<Vec<IxPair>>()
            };

        let inner_programs = if keys.take()[outer_instruction.program_id_index() as usize].0
            == mpl_bubblegum::ID.to_bytes()
        {
            let mut p = programs.clone();
            p.remove(spl_noop::ID.as_ref());
            p
        } else {
            programs.clone()
        };

        let hoister = non_hoisted_inner_instruction.clone();
        let hoisted = hoist_known_programs(&inner_programs, hoister);

        for h in hoisted {
            ordered_ixs.push_back(h);
        }

        {
            let kb = keys.borrow();
            let outer_ix_program_id_index = outer_instruction.program_id_index() as usize;
            let outer_program_id = kb.get(outer_ix_program_id_index);
            if outer_program_id.is_none() {
                eprintln!("outer program id deserialization error");
                continue;
            }
            let outer_program_id = **outer_program_id.unwrap();
            if inner_programs.get(outer_program_id.0.as_ref()).is_some() {
                ordered_ixs.push_back((
                    (outer_program_id, outer_instruction),
                    Some(non_hoisted_inner_instruction),
                ));
            }
        }
    }
    ordered_ixs
}

fn hoist_known_programs<'a, 'b>(
    programs: &'b HashSet<&'b [u8]>,
    instructions: Vec<(Pubkey, CompiledInstruction<'a>)>,
) -> Vec<(IxPair<'a>, Option<Vec<IxPair<'a>>>)> {
    let mut hoist = Vec::new();
    // there must be a safe and less copy way to do this, I should only need to move CI, and copy the found nodes matching predicate on 172
    for (index, (pid, ci)) in instructions.iter().enumerate() {
        let clone_for_inner = instructions.clone();

        if programs.get(pid.0.as_ref()).is_some() {
            let mut inner_copy = vec![];
            for new_inner_elem in clone_for_inner.into_iter().skip(index + 1) {
                if pid.0 != new_inner_elem.0 .0 {
                    inner_copy.push(new_inner_elem);
                } else {
                    break;
                }
            }

            hoist.push(((*pid, *ci), Some(inner_copy)));
        }
    }
    hoist
}
