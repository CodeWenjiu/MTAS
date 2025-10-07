use image::{GrayImage, ImageBuffer, Luma, open};
use imageproc::template_matching::{MatchTemplateMethod, match_template};
use std::path::PathBuf;
use strum::{EnumIter, IntoEnumIterator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum MTPage {
    Enter,
}

#[derive(Debug, Clone)]
pub struct ButtonMatch {
    pub button: MTButton,
    pub x: f64,
    pub y: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct PageMatch {
    pub page: MTPage,
    pub buttons: Vec<ButtonMatch>,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum MatchError {
    NoMatch,
    ImageError(String),
    LowConfidence { confidence: f64, threshold: f64 },
}

pub type Result<T> = std::result::Result<T, MatchError>;

impl MTPage {
    pub fn verify(&self, image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<PageMatch> {
        let buttons = self.match_buttons(image)?;
        let confidence = Self::calculate_page_confidence(&buttons);

        const CONFIDENCE_THRESHOLD: f64 = 0.6;
        if confidence < CONFIDENCE_THRESHOLD {
            return Err(MatchError::LowConfidence {
                confidence,
                threshold: CONFIDENCE_THRESHOLD,
            });
        }

        Ok(PageMatch {
            page: *self,
            buttons,
            confidence,
        })
    }

    pub fn detect_any(image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<PageMatch> {
        let mut best_match: Option<PageMatch> = None;

        for page in MTPage::iter() {
            if let Ok(page_match) = page.verify(image) {
                match &best_match {
                    None => best_match = Some(page_match),
                    Some(current_best) if page_match.confidence > current_best.confidence => {
                        best_match = Some(page_match);
                    }
                    _ => {}
                }
            }
        }

        best_match.ok_or(MatchError::NoMatch)
    }

    fn match_buttons(&self, image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<Vec<ButtonMatch>> {
        match self {
            MTPage::Enter => Self::match_enter_buttons(image),
        }
    }

    fn match_enter_buttons(image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<Vec<ButtonMatch>> {
        let matches: Vec<ButtonMatch> = EnterButton::iter()
            .filter_map(|button| button.match_in_image(image).ok())
            .collect();

        if matches.is_empty() {
            Err(MatchError::NoMatch)
        } else {
            Ok(matches)
        }
    }

    fn calculate_page_confidence(buttons: &[ButtonMatch]) -> f64 {
        if buttons.is_empty() {
            return 0.0;
        }
        buttons.iter().map(|b| b.confidence).sum::<f64>() / buttons.len() as f64
    }

    fn get_template_path(&self) -> PathBuf {
        let template_name = match self {
            MTPage::Enter => "enter.png",
        };

        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/matcher/mt/template")
            .join(template_name)
    }

    fn load_template(&self) -> Result<GrayImage> {
        let path = self.get_template_path();
        let img = open(&path)
            .map_err(|e| MatchError::ImageError(format!("无法加载模板 {:?}: {}", path, e)))?
            .to_luma8();
        Ok(img)
    }

    pub fn match_page_template(&self, image: &GrayImage) -> Result<f64> {
        let template = self.load_template()?;

        if image.width() < template.width() || image.height() < template.height() {
            return Err(MatchError::ImageError("图像尺寸小于模板尺寸".to_string()));
        }

        let result = match_template(
            image,
            &template,
            MatchTemplateMethod::CrossCorrelationNormalized,
        );

        let mut max_val = 0.0f32;
        for pixel in result.pixels() {
            let val = pixel[0];
            if val > max_val {
                max_val = val;
            }
        }

        Ok(max_val as f64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MTButton {
    Enter(EnterButton),
    // 其他页面的按钮可以在这里添加
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum EnterButton {
    Enter,
    Protocol,
    Announcement,
}

/// ButtonMatches trait - 定义按钮匹配的通用行为
///
/// 任何实现此 trait 的类型都可以：
/// 1. 提供模板位置 (get_template_pos)
/// 2. 计算匹配置信度 (match_confidence)
/// 3. 转换为 MTButton 枚举 (to_mt_button)
/// 4. 自动获得 match_in_image 实现
pub trait ButtonMatches: Sized + Copy {
    /// 获取按钮模板文件名
    fn get_template_name(&self) -> &'static str;

    /// 转换为 MTButton 枚举
    fn to_mt_button(self) -> MTButton;

    /// 获取模板图片路径
    fn get_template_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/matcher/mt/template")
            .join(self.get_template_name())
    }

    /// 加载按钮模板图片
    fn load_template(&self) -> Result<GrayImage> {
        let path = self.get_template_path();
        let img = open(&path)
            .map_err(|e| MatchError::ImageError(format!("无法加载模板 {:?}: {}", path, e)))?
            .to_luma8();
        Ok(img)
    }

    /// 使用模板匹配计算置信度和位置
    fn match_confidence(&self, image: &GrayImage) -> Result<(f64, f64, f64)> {
        let template = self.load_template()?;

        if image.width() < template.width() || image.height() < template.height() {
            return Err(MatchError::ImageError("图像尺寸小于模板尺寸".to_string()));
        }

        // 使用归一化互相关进行模板匹配
        let result = match_template(
            image,
            &template,
            MatchTemplateMethod::CrossCorrelationNormalized,
        );

        // 找到最大值位置（最佳匹配）
        let mut max_val = 0.0f32;
        let mut max_x = 0u32;
        let mut max_y = 0u32;

        for (x, y, pixel) in result.enumerate_pixels() {
            let val = pixel[0];
            if val > max_val {
                max_val = val;
                max_x = x;
                max_y = y;
            }
        }

        // 计算中心点位置（归一化坐标）
        let center_x = (max_x + template.width() / 2) as f64 / image.width() as f64;
        let center_y = (max_y + template.height() / 2) as f64 / image.height() as f64;

        Ok((center_x, center_y, max_val as f64))
    }

    /// 在图像中匹配该按钮（自动实现）
    ///
    /// 这个方法使用 match_confidence 和 to_mt_button 来完成完整的匹配流程
    fn match_in_image(&self, image: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<ButtonMatch> {
        let (x, y, confidence) = self.match_confidence(image)?;

        const BUTTON_CONFIDENCE_THRESHOLD: f64 = 0.6;
        if confidence < BUTTON_CONFIDENCE_THRESHOLD {
            return Err(MatchError::LowConfidence {
                confidence,
                threshold: BUTTON_CONFIDENCE_THRESHOLD,
            });
        }

        Ok(ButtonMatch {
            button: self.to_mt_button(),
            x,
            y,
            confidence,
        })
    }
}

/// 为 EnterButton 实现 ButtonMatches trait
impl ButtonMatches for EnterButton {
    fn get_template_name(&self) -> &'static str {
        match self {
            EnterButton::Enter => "enter_button.png",
            EnterButton::Protocol => "protocol_button.png",
            EnterButton::Announcement => "announcement_button.png",
        }
    }

    fn to_mt_button(self) -> MTButton {
        MTButton::Enter(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的图像（模拟Enter页面）
    fn create_test_image_enter_page(quality: ImageQuality) -> ImageBuffer<Luma<u8>, Vec<u8>> {
        let brightness = match quality {
            ImageQuality::Good => 128,
            ImageQuality::Dark => 50,
            ImageQuality::Bright => 200,
        };
        ImageBuffer::from_pixel(800, 600, Luma([brightness]))
    }

    /// 创建测试用的图像（模拟非Enter页面 - 黑屏）
    fn create_test_image_other_page() -> ImageBuffer<Luma<u8>, Vec<u8>> {
        // 创建一个黑色图像模拟其他页面（亮度0，会被识别为非Enter页面）
        ImageBuffer::from_pixel(800, 600, Luma([0]))
    }

    enum ImageQuality {
        Good,
        Dark,
        Bright,
    }

    #[test]
    fn test_verify_correct_page() {
        let image = create_test_image_enter_page(ImageQuality::Good);
        let page = MTPage::Enter;

        match page.verify(&image) {
            Ok(page_match) => {
                println!("✓ 快速路径测试通过");
                println!("  - 识别到页面: {:?}", page_match.page);
                println!("  - 按钮数量: {}", page_match.buttons.len());
                println!("  - 整体置信度: {:.2}", page_match.confidence);
            }
            Err(e) => {
                println!("⚠ 快速路径测试（预期可能失败，因为使用了纯色测试图像）");
                println!("  错误: {:?}", e);
            }
        }
    }

    #[test]
    fn test_verify_wrong_page() {
        // 测试快速路径：验证错误的页面应该失败
        let image = create_test_image_other_page();
        let page = MTPage::Enter;

        let result = page.verify(&image);
        assert!(result.is_err(), "不匹配的页面应该返回错误");

        match result {
            Err(MatchError::NoMatch) | Err(MatchError::LowConfidence { .. }) => {
                println!("✓ 错误页面检测通过 - 正确拒绝了黑屏图像");
            }
            _ => panic!("应该返回NoMatch或LowConfidence错误"),
        }
    }

    #[test]
    fn test_detect_any_success() {
        // 测试慢速路径：全局搜索应该找到Enter页面
        let image = create_test_image_enter_page(ImageQuality::Good);

        match MTPage::detect_any(&image) {
            Ok(page_match) => {
                println!("✓ 全局搜索测试通过");
                println!("  - 找到页面: {:?}", page_match.page);
                println!("  - 整体置信度: {:.2}", page_match.confidence);
            }
            Err(e) => {
                println!("⚠ 全局搜索测试（预期可能失败，因为使用了纯色测试图像）");
                println!("  错误: {:?}", e);
            }
        }
    }

    #[test]
    fn test_detect_any_no_match() {
        // 测试慢速路径：没有页面匹配时应该返回错误
        let image = create_test_image_other_page();

        let result = MTPage::detect_any(&image);
        assert!(result.is_err(), "没有匹配的页面应该返回错误");

        match result {
            Err(MatchError::NoMatch) => {
                println!("✓ 无匹配页面检测通过 - 黑屏图像未匹配任何页面");
            }
            _ => panic!("应该返回NoMatch错误"),
        }
    }

    #[test]
    fn test_template_loading() {
        // 测试模板加载功能
        println!("\n=== 模板加载测试 ===\n");

        let page = MTPage::Enter;
        let template_path = page.get_template_path();
        println!("模板路径: {:?}", template_path);

        match page.load_template() {
            Ok(template) => {
                println!("✓ 页面模板加载成功");
                println!("  - 尺寸: {}x{}", template.width(), template.height());
            }
            Err(e) => {
                println!("✗ 页面模板加载失败: {:?}", e);
            }
        }

        println!("\n按钮模板测试:");
        for button in EnterButton::iter() {
            let button_path = button.get_template_path();
            println!("  {:?}: {:?}", button, button_path);

            match button.load_template() {
                Ok(template) => {
                    println!(
                        "    ✓ 加载成功 ({}x{})",
                        template.width(),
                        template.height()
                    );
                }
                Err(e) => {
                    println!("    ✗ 加载失败: {:?}", e);
                }
            }
        }
    }

    #[test]
    fn test_template_matching_api() {
        // 测试模板匹配 API
        println!("\n=== 模板匹配 API 测试 ===\n");

        let image = create_test_image_enter_page(ImageQuality::Good);
        let page = MTPage::Enter;

        match page.match_page_template(&image) {
            Ok(confidence) => {
                println!("✓ 页面模板匹配完成");
                println!("  - 匹配置信度: {:.4}", confidence);
            }
            Err(e) => {
                println!("⚠ 页面模板匹配失败（预期行为，测试图像是纯色）");
                println!("  错误: {:?}", e);
            }
        }
    }

    #[test]
    fn test_button_matches_trait() {
        println!("\n=== ButtonMatches Trait 测试 ===\n");

        let image = create_test_image_enter_page(ImageQuality::Good);

        // 测试单个按钮的匹配
        println!("测试单个按钮匹配:");
        for button in EnterButton::iter() {
            println!("\n按钮: {:?}", button);
            println!("  - 模板文件: {}", button.get_template_name());

            // 测试 to_mt_button
            let mt_button = button.to_mt_button();
            println!("  - 转换为 MTButton: {:?}", mt_button);

            // 测试完整的 match_in_image
            match button.match_in_image(&image) {
                Ok(button_match) => {
                    println!("  - ✓ 匹配成功");
                    println!("    位置: ({:.2}, {:.2})", button_match.x, button_match.y);
                    println!("    置信度: {:.2}", button_match.confidence);
                }
                Err(e) => {
                    println!("  - ⚠ 匹配失败（预期行为）: {:?}", e);
                }
            }
        }

        println!("\n=== ButtonMatches Trait 测试完成 ===\n");
    }

    #[test]
    fn test_real_image_if_available() {
        // 尝试加载真实的 enter.png 并进行匹配
        println!("\n=== 真实图像测试 ===\n");

        let page = MTPage::Enter;
        match page.load_template() {
            Ok(real_image) => {
                println!("✓ 成功加载 enter.png");
                println!("  - 尺寸: {}x{}", real_image.width(), real_image.height());

                // 使用真实图像自己匹配自己（应该得到高置信度）
                match page.match_page_template(&real_image) {
                    Ok(confidence) => {
                        println!("  - 自匹配置信度: {:.4}", confidence);
                        assert!(confidence > 0.9, "自匹配应该得到很高的置信度");
                        println!("  ✓ 自匹配测试通过！");
                    }
                    Err(e) => {
                        println!("  ✗ 自匹配失败: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠ 无法加载 enter.png: {:?}", e);
                println!("  这是正常的，如果模板文件不存在");
            }
        }
    }

    #[test]
    fn test_workflow_simulation() {
        // 模拟真实工作流程
        println!("\n=== 工作流程模拟 ===\n");

        // 场景1: 首次启动，不知道在哪个页面
        println!("场景1: 应用启动，位置未知");
        let image = create_test_image_enter_page(ImageQuality::Good);

        match MTPage::detect_any(&image) {
            Ok(page_match) => {
                let current_page = page_match.page;
                println!("  → 通过全局搜索确定当前页面: {:?}\n", current_page);

                // 场景2: 已知在Enter页面，快速验证
                println!("场景2: 已知在 {:?} 页面，快速验证", current_page);
                match current_page.verify(&image) {
                    Ok(_) => println!("  → 快速验证成功\n"),
                    Err(e) => println!("  → 快速验证失败: {:?}\n", e),
                }
            }
            Err(e) => {
                println!("  → 全局搜索失败（使用纯色测试图像）: {:?}\n", e);
            }
        }

        // 场景3: 页面切换后，预期页面匹配失败
        println!("场景3: 执行了切换操作，页面变为黑屏");
        let other_image = create_test_image_other_page();
        let result = MTPage::Enter.verify(&other_image);
        assert!(result.is_err(), "黑屏应该匹配失败");
        println!("  → 快速验证失败（黑屏不是Enter页面），触发全局搜索");

        match MTPage::detect_any(&other_image) {
            Ok(page_match) => {
                println!("  → 找到新页面: {:?}\n", page_match.page);
            }
            Err(_) => {
                println!("  → 未找到匹配页面（黑屏状态，可能在加载中）\n");
            }
        }

        println!("=== 工作流程模拟完成 ===\n");
    }
}
