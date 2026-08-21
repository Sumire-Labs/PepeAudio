pub(crate) const ACTION_ROW: u8 = 1;
pub(crate) const BUTTON: u8 = 2;
pub(crate) const STRING_SELECT: u8 = 3;
pub(crate) const SECTION: u8 = 9;
pub(crate) const TEXT_DISPLAY: u8 = 10;
pub(crate) const THUMBNAIL: u8 = 11;
pub(crate) const SEPARATOR: u8 = 14;
pub(crate) const CONTAINER: u8 = 17;

pub(crate) const PRIMARY_BUTTON: u8 = 1;
pub(crate) const SECONDARY_BUTTON: u8 = 2;
pub(crate) const SUCCESS_BUTTON: u8 = 3;
pub(crate) const DANGER_BUTTON: u8 = 4;
pub(crate) const LINK_BUTTON: u8 = 5;
pub(crate) const MAX_COMPONENTS: usize = 40;
pub(crate) const MAX_BUTTONS_PER_ROW: usize = 5;
pub(crate) const MAX_BUTTON_LABEL_CHARS: usize = 80;
pub(crate) const MAX_BUTTON_URL_BYTES: usize = 512;
pub(crate) const MAX_SELECT_OPTIONS: usize = 25;
pub(crate) const MAX_CUSTOM_ID_CHARS: usize = 100;

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) const fn is_false(value: &bool) -> bool {
    !*value
}
