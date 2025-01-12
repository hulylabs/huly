// RebelDB™ © 2025 Huly Labs • https://hulylabs.com • SPDX-License-Identifier: MIT
//
// codegen.rs:

use crate::parser::ValueIterator;
use crate::value::Value;
use crate::{heap::Heap, value::Serialize};
use thiserror::Error;
use wasm_encoder::{
    CodeSection, Function, FunctionSection, Instruction, Module, TypeSection, ValType,
};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error(transparent)]
    ParseError(#[from] crate::parser::ParseError),
    #[error(transparent)]
    ValueError(#[from] crate::value::ValueError),
}

struct ConstantPool {
    offset: usize,
    data: Vec<u8>,
}

impl ConstantPool {
    fn new() -> Self {
        Self {
            offset: 0,
            data: Vec::new(),
        }
    }

    fn add_value(&mut self, value: Value) -> Result<i32, CompileError> {
        let offset = self.offset;
        let value_size = value.serialize(&mut self.data)?;
        self.offset += value_size;
        Ok(offset as i32)
    }
}

pub struct Compiler {
    module: Module,
    types: TypeSection,
    functions: FunctionSection,
    codes: CodeSection,
    constants: ConstantPool,
}

impl Compiler {
    const CONSTANTS_START: i32 = 0x1000;

    pub fn new() -> Self {
        Self {
            module: Module::new(),
            types: TypeSection::new(),
            functions: FunctionSection::new(),
            codes: CodeSection::new(),
            constants: ConstantPool::new(),
        }
    }

    pub fn make_function<T>(
        &mut self,
        params: Vec<ValType>,
        results: Vec<ValType>,
        body: ValueIterator<'_, T>,
    ) -> Result<(), CompileError>
    where
        T: Heap,
    {
        //self.types.ty().function(params, results);

        let locals = vec![];
        let mut func = Function::new(locals);

        for value in body {
            let value = value?;
            match value {
                Value::I32(i) => func.instruction(&Instruction::I32Const(i)),
                Value::I64(i) => func.instruction(&Instruction::I64Const(i)),
                Value::F32(f) => func.instruction(&Instruction::F32Const(f)),
                Value::F64(f) => func.instruction(&Instruction::F64Const(f)),
                Value::Bytes(enc, content) => {
                    let offset = self.constants.add_value(Value::Bytes(enc, content))?;
                    func.instruction(&Instruction::I32Const(Self::CONSTANTS_START + offset))
                }
                _ => unimplemented!(),
            };
        }

        Ok(())
    }
}
