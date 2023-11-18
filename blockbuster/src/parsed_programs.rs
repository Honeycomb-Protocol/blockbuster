pub enum Program {
    Bubblegum {
        parser: bubblegum::BubblegumParser,
        instruction_result: BubblegumInstruction,
        account_result: (),
    },
    AccountCompression {
        parser: account_compression::AccountCompressionParser,
        instruction_result: AccountCompressionInstruction,
        account_result: (),
    },
}

impl ProgramParser for Program {}
