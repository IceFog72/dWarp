//! In-app notifications for third-party CLI agent sessions (toasts and the mailbox).

mod item;
pub(crate) mod item_rendering;
mod model;
pub(crate) mod toast_stack;
pub(crate) mod view;

pub(crate) use item::{
    NotificationCategory, NotificationFilter, NotificationId, NotificationItem, NotificationItems,
    NotificationOrigin, NotificationSourceAgent,
};
pub(crate) use model::{AgentManagementEvent, AgentNotificationsModel};

pub fn init(app: &mut warpui::AppContext) {
    view::NotificationMailboxView::init(app);
}
