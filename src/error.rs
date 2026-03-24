use std::process;

pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_NO_DUPES: i32 = 0;
pub const EXIT_DUPES_FOUND: i32 = 1;
pub const EXIT_ERROR: i32 = 2;

pub fn exit_with(code: i32) -> ! {
    process::exit(code)
}
