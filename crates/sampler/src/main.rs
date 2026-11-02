//! `glowden-sampler`: the screen's average colors on a grid, for the screen sampler lighting
//! effect. The work is in `sampler.c`.

use std::ffi::{CString, c_char, c_int};

unsafe extern "C" {
    fn glowden_sampler_main(argc: c_int, argv: *mut *mut c_char) -> c_int;
}

fn main() {
    let args: Vec<CString> = std::env::args()
        .map(|a| CString::new(a).expect("argument contains a NUL byte"))
        .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());
    let code = unsafe { glowden_sampler_main(args.len() as c_int, argv.as_mut_ptr()) };
    std::process::exit(code);
}
