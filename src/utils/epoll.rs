use std::os::unix::io::RawFd;
use std::time::Duration;

#[derive(Debug)]
pub struct EpollEvent {
    pub fd: RawFd,
    pub readable: bool,
    pub writable: bool,
}

pub struct EpollManager {
    epoll_fd: RawFd,
}

impl EpollManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd == -1 {
            return Err("Failed to create epoll instance".into());
        }
        
        Ok(EpollManager { epoll_fd })
    }

    pub fn add_listener(&self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let mut event = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: fd as u64,
        };
        
        let result = unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event) };
        if result == -1 {
            return Err("Failed to add listener to epoll".into());
        }
        
        Ok(())
    }

    pub fn add_client(&self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let mut event = libc::epoll_event {
            events: (libc::EPOLLIN | libc::EPOLLOUT | libc::EPOLLET) as u32,
            u64: fd as u64,
        };
        
        let result = unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event) };
        if result == -1 {
            return Err("Failed to add client to epoll".into());
        }
        
        Ok(())
    }

    pub fn remove_client(&self, fd: RawFd) -> Result<(), Box<dyn std::error::Error>> {
        let result = unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        if result == -1 {
            return Err("Failed to remove client from epoll".into());
        }
        
        Ok(())
    }

    pub fn wait(&self, timeout: Duration) -> Result<Vec<EpollEvent>, Box<dyn std::error::Error>> {
        const MAX_EVENTS: usize = 64;
        let mut events: [libc::epoll_event; MAX_EVENTS] = unsafe { std::mem::zeroed() };
        
        let timeout_ms = timeout.as_millis() as i32;
        let num_events = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                events.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_ms,
            )
        };
        
        if num_events == -1 {
            return Err("epoll_wait failed".into());
        }
        
        let mut result = Vec::new();
        for i in 0..num_events as usize {
            let event = &events[i];
            let fd = event.u64 as RawFd;
            let readable = (event.events & libc::EPOLLIN as u32) != 0;
            let writable = (event.events & libc::EPOLLOUT as u32) != 0;
            
            result.push(EpollEvent {
                fd,
                readable,
                writable,
            });
        }
        
        Ok(result)
    }
}

impl Drop for EpollManager {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.epoll_fd);
        }
    }
}