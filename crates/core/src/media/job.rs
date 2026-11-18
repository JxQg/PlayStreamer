use anyhow::Result;
use tokio::process::Child;

#[cfg(windows)]
pub(super) struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
// The owned kernel handle is thread-safe; it is closed exactly once by Drop.
unsafe impl Send for ProcessJob {}
#[cfg(windows)]
unsafe impl Sync for ProcessJob {}

#[cfg(windows)]
impl ProcessJob {
    pub fn attach(child: &Child) -> Result<Self> {
        use anyhow::{Context, anyhow};
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
        };
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("无法创建媒体进程生命周期容器");
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            )
        };
        if set == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                CloseHandle(handle);
            }
            return Err(error).context("无法配置媒体进程退出策略");
        }
        let process = child
            .raw_handle()
            .ok_or_else(|| anyhow!("无法获取媒体进程句柄"));
        let process = match process {
            Ok(process) => process,
            Err(error) => {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(error);
            }
        };
        if unsafe { AssignProcessToJobObject(handle, process as _) } == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                CloseHandle(handle);
            }
            return Err(error).context("无法关联媒体进程生命周期");
        }
        Ok(Self(handle))
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
pub(super) struct ProcessJob;
#[cfg(not(windows))]
impl ProcessJob {
    pub fn attach(_: &Child) -> Result<Self> {
        Ok(Self)
    }
}
