#[no_mangle]
pub extern "C" fn c_init_sdk() {
    crate::init_sdk();
}

#[no_mangle]
pub extern "C" fn c_close_sdk() {
    crate::close_sdk();
}
