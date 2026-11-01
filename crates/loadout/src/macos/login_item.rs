//! Open at login through `SMAppService` (bundled app only).

use anyhow::{Result, anyhow};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginItem {
    Off,
    On,
    /// Registered, but the user must allow it in System Settings → Login Items.
    NeedsApproval,
}

pub fn status() -> LoginItem {
    // SAFETY: mainAppService and status have no preconditions.
    match unsafe { SMAppService::mainAppService().status() } {
        SMAppServiceStatus::Enabled => LoginItem::On,
        SMAppServiceStatus::RequiresApproval => LoginItem::NeedsApproval,
        _ => LoginItem::Off,
    }
}

pub fn set(on: bool) -> Result<LoginItem> {
    // SAFETY: register/unregister have no preconditions; failures come back as NSError.
    let result = unsafe {
        let service = SMAppService::mainAppService();
        if on {
            service.registerAndReturnError()
        } else {
            service.unregisterAndReturnError()
        }
    };
    result.map_err(|err| anyhow!("{}", err.localizedDescription()))?;
    Ok(status())
}
