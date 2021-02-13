mod helpers;
mod instructions;
mod layout;
mod writer;

#[cfg(test)]
mod test_framework;

#[cfg(test)]
mod layout_tests;

pub use spirv::Capability;
pub use writer::{Error, Writer};

use spirv::Word;

bitflags::bitflags! {
    pub struct WriterFlags: u32 {
        const NONE = 0x0;
        const DEBUG = 0x1;
    }
}

struct PhysicalLayout {
    magic_number: Word,
    version: Word,
    generator: Word,
    bound: Word,
    instruction_schema: Word,
}

#[derive(Default)]
struct LogicalLayout {
    capabilities: Vec<Word>,
    extensions: Vec<Word>,
    ext_inst_imports: Vec<Word>,
    memory_model: Vec<Word>,
    entry_points: Vec<Word>,
    execution_modes: Vec<Word>,
    debugs: Vec<Word>,
    annotations: Vec<Word>,
    declarations: Vec<Word>,
    function_declarations: Vec<Word>,
    function_definitions: Vec<Word>,
}

struct Instruction {
    op: spirv::Op,
    wc: u32,
    type_id: Option<Word>,
    result_id: Option<Word>,
    operands: Vec<Word>,
}

pub fn write_vec(
    module: &crate::Module,
    flags: WriterFlags,
    capabilities: crate::FastHashSet<spirv::Capability>,
) -> Result<Vec<u32>, Error> {
    let mut words = Vec::new();
    let mut w = Writer::new(&module.header, flags, capabilities);
    w.write(module, &mut words)?;
    Ok(words)
}
