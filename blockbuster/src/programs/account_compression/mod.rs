use log::warn;

use crate::{
    error::BlockbusterError,
    instruction::InstructionBundle,
    program_handler::{ParseResult, ProgramParser},
};

use crate::{program_handler::NotUsed, programs::ProgramParseResult};
use borsh::de::BorshDeserialize;
use plerkle_serialization::AccountInfo;
use solana_sdk::pubkey::Pubkey;
pub use spl_account_compression::events::{
    AccountCompressionEvent::{self, ApplicationData, ChangeLog},
    ApplicationDataEvent, ApplicationDataEventV1, ChangeLogEvent, ChangeLogEventV1,
};

use spl_account_compression::id as program_id;
use spl_noop;

use anchor_lang::Discriminator;
use spl_account_compression::instruction::InitEmptyMerkleTree;

#[derive(Eq, PartialEq)]
pub enum Instruction {
    Unknown,
    InitTree {
        max_depth: u32,
        max_buffer_size: u32,
    },
    ReplaceLeaf {
        root: [u8; 32],
        previous_leaf: [u8; 32],
        new_leaf: [u8; 32],
        index: u32,
    },
    TransferAuthority {
        new_authority: Pubkey,
    },
    VerifyLeaf {
        root: [u8; 32],
        leaf: [u8; 32],
        index: u32,
    },
    Append {
        leaf: [u8; 32],
    },
    InsertOrAppend {
        root: [u8; 32],
        leaf: [u8; 32],
        index: u32,
    },
    CloseTree,
}
//TODO add more of the parsing here to minimize program transformer code
pub struct AccountCompressionInstruction {
    pub instruction: Instruction,
    pub tree_update: Option<ChangeLogEventV1>,
    pub leaf_update: Option<ApplicationDataEventV1>,
    // pub payload: Option<Payload>,
}

impl AccountCompressionInstruction {
    pub fn new(ix: Instruction) -> Self {
        InitEmptyMerkleTree::DISCRIMINATOR;
        AccountCompressionInstruction {
            instruction: ix,
            tree_update: None,
            leaf_update: None,
            // payload: None,
        }
    }
}

impl ParseResult for AccountCompressionInstruction {
    fn result_type(&self) -> ProgramParseResult {
        ProgramParseResult::AccountCompression(self)
    }
    fn result(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

pub struct AccountCompressionParser;

impl ProgramParser for AccountCompressionParser {
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
            // instruction,
            inner_ix,
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
        let ix_type = get_instruction_type(&outer_ix_data);
        let mut b_inst = AccountCompressionInstruction::new();
        if let Some(ixs) = inner_ix {
            for ix in ixs {
                if ix.0 .0 == spl_noop::id().to_bytes() {
                    let cix = ix.1;
                    if let Some(inner_ix_data) = cix.data() {
                        let inner_ix_data = inner_ix_data.iter().collect::<Vec<_>>();
                        if !inner_ix_data.is_empty() {
                            match AccountCompressionEvent::try_from_slice(&inner_ix_data) {
                                Ok(result) => match result {
                                    ChangeLog(changelog_event) => {
                                        let ChangeLogEvent::V1(changelog_event) = changelog_event;
                                        b_inst.tree_update = Some(changelog_event);
                                    }
                                    ApplicationData(app_data) => {
                                        let ApplicationDataEvent::V1(app_data) = app_data;
                                        b_inst.leaf_update = Some(app_data);
                                    }
                                },
                                Err(e) => {
                                    warn!(
                                        "Error while deserializing txn {:?} with noop data: {:?}",
                                        txn_id, e
                                    );
                                }
                            }
                        }
                    } else {
                        return Err(BlockbusterError::InstructionParsingError);
                    }
                }
            }
        }

        // if outer_ix_data.len() >= 8 {
        //     let ix_data = &outer_ix_data[8..];
        //     if !ix_data.is_empty() {
        //         match b_inst.instruction {
        //             InstructionName::MintV1 => {
        //                 let args: MetadataArgs = MetadataArgs::try_from_slice(ix_data)?;
        //                 b_inst.payload = Some(Payload::MintV1 { args });
        //             }
        //             InstructionName::MintToCollectionV1 => {
        //                 let mut args: MetadataArgs = MetadataArgs::try_from_slice(ix_data)?;
        //                 if let Some(ref mut col) = args.collection {
        //                     col.verified = true;
        //                 }
        //                 b_inst.payload = Some(Payload::MintV1 { args });
        //             }
        //             InstructionName::DecompressV1 => {
        //                 let args: MetadataArgs = MetadataArgs::try_from_slice(ix_data)?;
        //                 b_inst.payload = Some(Payload::Decompress { args });
        //             }
        //             InstructionName::CancelRedeem => {
        //                 let slice: [u8; 32] = ix_data
        //                     .try_into()
        //                     .map_err(|_e| BlockbusterError::InstructionParsingError)?;
        //                 b_inst.payload = Some(Payload::CancelRedeem { root: slice });
        //             }
        //             InstructionName::VerifyCreator => {
        //                 b_inst.payload = Some(build_creator_verification_payload(keys, true)?);
        //             }
        //             InstructionName::UnverifyCreator => {
        //                 b_inst.payload = Some(build_creator_verification_payload(keys, false)?);
        //             }
        //             InstructionName::VerifyCollection | InstructionName::SetAndVerifyCollection => {
        //                 b_inst.payload = Some(build_collection_verification_payload(keys, true)?);
        //             }
        //             InstructionName::UnverifyCollection => {
        //                 b_inst.payload = Some(build_collection_verification_payload(keys, false)?);
        //             }
        //             InstructionName::Unknown => {}
        //             _ => {}
        //         };
        //     }
        // }

        Ok(Box::new(b_inst))
    }
}

// // See AccountCompression documentation for offsets and positions:
// // https://github.com/metaplex-foundation/mpl-AccountCompression/blob/main/programs/AccountCompression/README.md#-verify_creator-and-unverify_creator
// fn build_creator_verification_payload(
//     keys: &[plerkle_serialization::Pubkey],
//     verify: bool,
// ) -> Result<Payload, BlockbusterError> {
//     let creator = keys
//         .get(5)
//         .ok_or(BlockbusterError::InstructionParsingError)?
//         .0;
//     Ok(Payload::CreatorVerification {
//         creator: Pubkey::new_from_array(creator),
//         verify,
//     })
// }

// // See AccountCompression for offsets and positions:
// // https://github.com/metaplex-foundation/mpl-AccountCompression/blob/main/programs/AccountCompression/README.md#-verify_collection-unverify_collection-and-set_and_verify_collection
// // NOTE: Unverfication does not include collection. This needs to be fixed in the README.
// fn build_collection_verification_payload(
//     keys: &[plerkle_serialization::Pubkey],
//     verify: bool,
// ) -> Result<Payload, BlockbusterError> {
//     let collection_raw = keys
//         .get(8)
//         .ok_or(BlockbusterError::InstructionParsingError)?
//         .0;
//     let collection: Pubkey = Pubkey::try_from_slice(&collection_raw)?;
//     Ok(Payload::CollectionVerification { collection, verify })
// }
