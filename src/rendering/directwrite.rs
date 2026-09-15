use std::sync::Arc;
use windows::core::{w, Result};
use windows::Win32::Graphics::DirectWrite::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyRole {
    AppTitle,
    CardHeader,
    MetricLarge,
    MetricUnit,
    Body,
    Caption,
    Monospace,
    LcdCenterBig,
    LcdCenterSmall,
}

pub struct DirectWriteContext {
    pub factory: IDWriteFactory,
    pub title_format: IDWriteTextFormat,
    pub card_header_format: IDWriteTextFormat,
    pub metric_large_format: IDWriteTextFormat,
    pub metric_unit_format: IDWriteTextFormat,
    pub body_format: IDWriteTextFormat,
    pub caption_format: IDWriteTextFormat,
    pub mono_format: IDWriteTextFormat,
    pub lcd_center_big_format: IDWriteTextFormat,
    pub lcd_center_small_format: IDWriteTextFormat,
}

impl DirectWriteContext {
    pub fn new() -> Result<Arc<Self>> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            let title_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                20.0,
                w!("en-US"),
            )?;

            let card_header_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                13.0,
                w!("en-US"),
            )?;

            let metric_large_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                22.0,
                w!("en-US"),
            )?;

            let metric_unit_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                11.0,
                w!("en-US"),
            )?;

            let body_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0,
                w!("en-US"),
            )?;

            let caption_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                10.0,
                w!("en-US"),
            )?;

            let mono_format = factory.CreateTextFormat(
                w!("Consolas"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                11.0,
                w!("en-US"),
            )?;

            let lcd_center_big_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                24.0,
                w!("en-US"),
            )?;
            lcd_center_big_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            lcd_center_big_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            let lcd_center_small_format = factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                11.0,
                w!("en-US"),
            )?;
            lcd_center_small_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            lcd_center_small_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(Arc::new(Self {
                factory,
                title_format,
                card_header_format,
                metric_large_format,
                metric_unit_format,
                body_format,
                caption_format,
                mono_format,
                lcd_center_big_format,
                lcd_center_small_format,
            }))
        }
    }

    pub fn format_for_role(&self, role: TypographyRole) -> &IDWriteTextFormat {
        match role {
            TypographyRole::AppTitle => &self.title_format,
            TypographyRole::CardHeader => &self.card_header_format,
            TypographyRole::MetricLarge => &self.metric_large_format,
            TypographyRole::MetricUnit => &self.metric_unit_format,
            TypographyRole::Body => &self.body_format,
            TypographyRole::Caption => &self.caption_format,
            TypographyRole::Monospace => &self.mono_format,
            TypographyRole::LcdCenterBig => &self.lcd_center_big_format,
            TypographyRole::LcdCenterSmall => &self.lcd_center_small_format,
        }
    }
}
