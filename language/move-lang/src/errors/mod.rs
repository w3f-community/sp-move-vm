// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use alloc::vec::Vec;
use move_ir_types::location::Loc;
use std::collections::HashMap;

//**************************************************************************************************
// Types
//**************************************************************************************************

pub type Errors = Vec<Error>;
pub type Error = Vec<(Loc, String)>;
pub type ErrorSlice = [(Loc, String)];
pub type HashableError = Vec<(&'static str, usize, usize, String)>;

pub type FilesSourceText = HashMap<&'static str, String>;

//**************************************************************************************************
// Utils
//**************************************************************************************************

pub fn check_errors(errors: Errors) -> Result<(), Errors> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
