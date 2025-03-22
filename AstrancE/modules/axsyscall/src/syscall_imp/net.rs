use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;

pub fn ax_socket(domain: c_int, socktype: c_int, protocol: c_int) -> Result<isize,isize>{
    let ret = api::sys_socket(domain, socktype, protocol) as isize;
    if ret < 0 {
        Err(ret)
    }else {
        Ok(ret)
    }
}

pub fn ax_bind(
    socket_fd: c_int,
    addr: *const ctypes::sockaddr,
    addrlen: ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_bind(socket_fd, addr, addrlen) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}

// Socket connection
pub fn ax_connect(
    socket_fd: c_int,
    addr: *const ctypes::sockaddr,
    addrlen: ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_connect(socket_fd, addr, addrlen) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}

// Data sending with address
pub fn ax_sendto(
    socket_fd: c_int,
    buf: *const c_void,
    len: usize,
    _flag: c_int,
    addr: *const ctypes::sockaddr,
    addrlen: ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_sendto(socket_fd, buf, len, 0, addr, addrlen) as isize;
    if ret < 0 { Err(ret) } else { Ok(ret) }
}

// Data sending (connected socket)
pub fn ax_send(
    socket_fd: c_int,
    buf: *const c_void,
    len: usize,
    _flag: c_int,
) -> Result<isize, isize> {
    let ret = api::sys_send(socket_fd, buf, len, 0) as isize;
    if ret < 0 { Err(ret) } else { Ok(ret) }
}

// Data receiving with address
pub unsafe fn ax_recvfrom(
    socket_fd: c_int,
    buf: *mut c_void,
    len: usize,
    _flag: c_int,
    addr: *mut ctypes::sockaddr,
    addrlen: *mut ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_recvfrom(socket_fd, buf, len, 0, addr, addrlen) as isize;
    if ret < 0 { Err(ret) } else { Ok(ret) }
}

// Data receiving (connected socket)
pub fn ax_recv(
    socket_fd: c_int,
    buf: *mut c_void,
    len: usize,
    _flag: c_int,
) -> Result<isize, isize> {
    let ret = api::sys_recv(socket_fd, buf, len, 0) as isize;
    if ret < 0 { Err(ret) } else { Ok(ret) }
}

// Socket listening
pub fn ax_listen(socket_fd: c_int, backlog: c_int) -> Result<isize, isize> {
    let ret = api::sys_listen(socket_fd, backlog) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}

// Connection acceptance
pub unsafe fn ax_accept(
    socket_fd: c_int,
    addr: *mut ctypes::sockaddr,
    addrlen: *mut ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_accept(socket_fd, addr, addrlen) as isize;
    if ret < 0 { Err(ret) } else { Ok(ret) }
}

// Socket shutdown
pub fn ax_shutdown(socket_fd: c_int, _how: c_int) -> Result<isize, isize> {
    let ret = api::sys_shutdown(socket_fd, 0) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}

// Socket address query
pub unsafe fn ax_getsockname(
    socket_fd: c_int,
    addr: *mut ctypes::sockaddr,
    addrlen: *mut ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_getsockname(socket_fd, addr, addrlen) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}

// Peer address query
pub unsafe fn ax_getpeername(
    socket_fd: c_int,
    addr: *mut ctypes::sockaddr,
    addrlen: *mut ctypes::socklen_t,
) -> Result<isize, isize> {
    let ret = api::sys_getpeername(socket_fd, addr, addrlen) as isize;
    if ret == 0 { Ok(0) } else { Err(ret) }
}