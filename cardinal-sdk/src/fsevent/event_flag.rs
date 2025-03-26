#![allow(non_upper_case_globals)]
use bitflags::bitflags;
bitflags! {
    pub struct EventFlag: u32 {
        const None = fsevent_sys::kFSEventStreamEventFlagNone;
        const MustScanSubDirs = fsevent_sys::kFSEventStreamEventFlagMustScanSubDirs;
        const UserDropped = fsevent_sys::kFSEventStreamEventFlagUserDropped;
        const KernelDropped = fsevent_sys::kFSEventStreamEventFlagKernelDropped;
        const EventIdsWrapped = fsevent_sys::kFSEventStreamEventFlagEventIdsWrapped;
        const HistoryDone = fsevent_sys::kFSEventStreamEventFlagHistoryDone;
        const RootChanged = fsevent_sys::kFSEventStreamEventFlagRootChanged;
        const Mount = fsevent_sys::kFSEventStreamEventFlagMount;
        const Unmount = fsevent_sys::kFSEventStreamEventFlagUnmount;
        const ItemCreated = fsevent_sys::kFSEventStreamEventFlagItemCreated;
        const ItemRemoved = fsevent_sys::kFSEventStreamEventFlagItemRemoved;
        const ItemInodeMetaMod = fsevent_sys::kFSEventStreamEventFlagItemInodeMetaMod;
        const ItemRenamed = fsevent_sys::kFSEventStreamEventFlagItemRenamed;
        const ItemModified = fsevent_sys::kFSEventStreamEventFlagItemModified;
        const ItemFinderInfoMod = fsevent_sys::kFSEventStreamEventFlagItemFinderInfoMod;
        const ItemChangeOwner = fsevent_sys::kFSEventStreamEventFlagItemChangeOwner;
        const ItemXattrMod = fsevent_sys::kFSEventStreamEventFlagItemXattrMod;
        const ItemIsFile = fsevent_sys::kFSEventStreamEventFlagItemIsFile;
        const ItemIsDir = fsevent_sys::kFSEventStreamEventFlagItemIsDir;
        const ItemIsSymlink = fsevent_sys::kFSEventStreamEventFlagItemIsSymlink;
        const OwnEvent = fsevent_sys::kFSEventStreamEventFlagOwnEvent;
        const IsHardlink = fsevent_sys::kFSEventStreamEventFlagItemIsHardlink;
        const IsLastHardlink = fsevent_sys::kFSEventStreamEventFlagItemIsLastHardlink;
        const Cloned = fsevent_sys::kFSEventStreamEventFlagItemCloned;
    }
}

pub enum EventType {
    Unknown,
    File,
    Dir,
    Symlink,
    Hardlink,
}

pub enum ScanType {
    SingleNode,
    Folder,
    /// Something wrong happened, do re-indexing.
    ReScan,
    /// Do nothing, since event id is always updated.
    Nop,
}

impl EventFlag {
    pub fn event_type(&self) -> EventType {
        if self.contains(EventFlag::IsHardlink) | self.contains(EventFlag::IsLastHardlink) {
            EventType::Hardlink
        } else if self.contains(EventFlag::ItemIsSymlink) {
            EventType::Symlink
        } else if self.contains(EventFlag::ItemIsDir) {
            EventType::Dir
        } else if self.contains(EventFlag::ItemIsFile) {
            EventType::File
        } else {
            EventType::Unknown
        }
    }

    pub fn scan_type(&self) -> ScanType {
        let event_type = self.event_type();
        let is_dir = matches!(event_type, EventType::Dir);
        if self.contains(EventFlag::None) {
            // Strange event, doesn't know when it happens, processing it using a generic way
            // e.g. new event: fs_event=FsEvent { path: "/.docid/16777229/changed/782/src=0,dst=41985052", flag: kFSEventStreamEventFlagNone, id: 471533015 }
            if is_dir {
                ScanType::Folder
            } else {
                ScanType::SingleNode
            }
        } else if self.contains(EventFlag::MustScanSubDirs)
            | self.contains(EventFlag::UserDropped)
            | self.contains(EventFlag::KernelDropped)
        {
            ScanType::ReScan
        } else if self.contains(EventFlag::EventIdsWrapped) | self.contains(EventFlag::HistoryDone)
        {
            ScanType::Nop
        } else if self.contains(EventFlag::RootChanged) {
            // Should never happen since we are watching "/"
            assert!(false);
            ScanType::ReScan
        } else if self.contains(EventFlag::Unmount) | self.contains(EventFlag::Mount) {
            assert!(is_dir);
            ScanType::Folder
        } else if self.contains(EventFlag::ItemCreated) {
            // creating dir is also single node
            ScanType::SingleNode
        } else if self.contains(EventFlag::ItemRemoved) {
            if is_dir {
                ScanType::Folder
            } else {
                ScanType::SingleNode
            }
        } else if self.contains(EventFlag::ItemInodeMetaMod) {
            // creating dir is also single node
            ScanType::SingleNode
        } else if self.contains(EventFlag::ItemRenamed) {
            if is_dir {
                ScanType::Folder
            } else {
                ScanType::SingleNode
            }
        } else if self.contains(EventFlag::ItemModified) {
            assert!(!is_dir);
            ScanType::SingleNode
        } else if self.contains(EventFlag::ItemFinderInfoMod)
            | self.contains(EventFlag::ItemChangeOwner)
            | self.contains(EventFlag::ItemXattrMod)
        {
            // creating dir is also single node
            ScanType::SingleNode
        } else if self.contains(EventFlag::OwnEvent) {
            unreachable!()
        } else if self.contains(EventFlag::Cloned) {
            if is_dir {
                ScanType::Folder
            } else {
                ScanType::SingleNode
            }
        } else {
            panic!("unexpected event: {:?}", self)
        }
    }

    pub fn event_action(&self) -> EventAction {
        let f = self;
        if f.contains(EventFlag::ItemCreated)
            || f.contains(EventFlag::ItemRemoved)
            || f.contains(EventFlag::Unmount)
            || f.contains(EventFlag::ItemInodeMetaMod)
            || f.contains(EventFlag::ItemXattrMod)
            || f.contains(EventFlag::ItemChangeOwner)
            || f.contains(EventFlag::ItemFinderInfoMod)
            || f.contains(EventFlag::ItemModified)
            // Nowhere to distinguish it's 'from' or 'to'.
            || f.contains(EventFlag::ItemRenamed)
            // Nowhere to distinguish it's 'from' or 'to'.
            || f.contains(EventFlag::Cloned)
        {
            EventAction::Modify
        } else {
            EventAction::Unknown
        }
    }
}

/// Abstract action of a file system event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventAction {
    Modify,
    Unknown,
}
