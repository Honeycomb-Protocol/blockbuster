use borsh::BorshDeserialize;
use log::warn;

use crate::{
    error::BlockbusterError,
    instruction::InstructionBundle,
    program_handler::{ParseResult, ProgramParser},
};

use crate::{program_handler::NotUsed, programs::ProgramParseResult};
use plerkle_serialization::AccountInfo;
use solana_sdk::pubkey::Pubkey;
pub use spl_account_compression::events::{
    AccountCompressionEvent::{self, ApplicationData, ChangeLog},
    ApplicationDataEvent, ApplicationDataEventV1, ChangeLogEvent, ChangeLogEventV1,
};

use spl_noop::id as program_id;

pub struct NoopInstruction {
    pub tree_update: Option<ChangeLogEventV1>,
    pub application_data: Option<ApplicationDataEventV1>,
}

impl NoopInstruction {
    pub fn new() -> Self {
        NoopInstruction {
            tree_update: None,
            application_data: None,
        }
    }
}

impl ParseResult for NoopInstruction {
    fn result_type(&self) -> ProgramParseResult {
        ProgramParseResult::Noop(self)
    }
    fn result(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

pub struct NoopParser;

impl ProgramParser for NoopParser {
    fn key(&self) -> Pubkey {
        program_id()
    }

    fn key_match(&self, key: &Pubkey) -> bool {
        key == &program_id()
    }
    fn handles_account_updates(&self) -> bool {
        false
    }

    fn handles_instructions(&self) -> bool {
        true
    }
    fn handle_account(
        &self,
        _account_info: &AccountInfo,
    ) -> Result<Box<(dyn ParseResult + 'static)>, BlockbusterError> {
        Ok(Box::new(NotUsed::new()))
    }

    fn handle_instruction(
        &self,
        bundle: &InstructionBundle,
    ) -> Result<Box<(dyn ParseResult + 'static)>, BlockbusterError> {
        let InstructionBundle {
            txn_id,
            instruction,
            // keys,
            ..
        } = bundle;
        let outer_ix_data = match instruction {
            Some(compiled_ix) if compiled_ix.data().is_some() => {
                let data = compiled_ix.data().unwrap();
                data.iter().collect::<Vec<_>>()
            }
            _ => {
                return Err(BlockbusterError::DeserializationError);
            }
        };

        if outer_ix_data.is_empty() {
            return Err(BlockbusterError::InstructionParsingError);
        }

        let mut b_inst = NoopInstruction::new();
        match AccountCompressionEvent::try_from_slice(&outer_ix_data) {
            Ok(result) => match result {
                ChangeLog(changelog_event) => {
                    let ChangeLogEvent::V1(changelog_event) = changelog_event;
                    b_inst.tree_update = Some(changelog_event);
                }
                ApplicationData(app_data) => {
                    let ApplicationDataEvent::V1(app_data) = app_data;
                    b_inst.application_data = Some(app_data);
                }
            },
            Err(e) => {
                warn!(
                    "Error while deserializing txn {:?} with noop data: {:?}",
                    txn_id, e
                );
            }
        }
        Ok(Box::new(b_inst))
    }
}
