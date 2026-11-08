use anyhow::{Result, bail};

pub(crate) trait TextClipboard {
    fn set_text(&mut self, text: &str) -> Result<()>;
}

#[derive(Default)]
pub(crate) struct SystemClipboard {
    // Linux serves copied text while this handle remains alive.
    clipboard: Option<arboard::Clipboard>,
}

impl TextClipboard for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        if text.contains('\0') {
            bail!("Selected text contains a null character.");
        }
        let clipboard = match &mut self.clipboard {
            Some(clipboard) => clipboard,
            None => self.clipboard.insert(arboard::Clipboard::new()?),
        };
        clipboard.set_text(text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_text_is_rejected_without_initializing_the_system_clipboard() {
        let mut clipboard = SystemClipboard::default();
        assert!(clipboard.clipboard.is_none());
        let error = clipboard.set_text("before\0after").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Selected text contains a null character."
        );
        assert!(clipboard.clipboard.is_none());
    }
}
