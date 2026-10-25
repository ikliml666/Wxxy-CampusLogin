//! 算术验证码识别：颜色不变强度图 → 垂直投影切分 → bbox 锚定 16×14 画布 → NCC 最近邻。
//!
//! 题面形态（REPORT.md 实测 + 100 张样本逐字符核对）：「左操作数 运算符(+/-/*) 右操作数 =」，
//! PNG 100×25，= 恒为第 4 片（忽略），片序 = [d1, op, d2]。
//!
//! 关键事实（100 张样本标定）：该校 Kaptcha 的**渲染位置完全确定**——每个槽位的字符
//! bbox 逐位一致（数字恒 y=5..18、左操作数右对齐 x1=10、右操作数 x1=60；`*` 恒
//! (26,33,5,13)；`+` 恒 (27,39,6,18)），无噪点、无旋转、无扭曲、单字体。故：
//! - **颜色必须线性归一化**（字符随机着色，275/300 种主色；`dist = 765 - Σrgb` 与颜色
//!   成线性比例，除以本图峰值即颜色不变）——早期用绝对阈值+亮度判定会丢浅彩字符与
//!   `*` 的细对角臂。
//! - **锚定原生 bbox 形状**（16×14 补零画布，非等比缩放）——早期 24×24 等比缩放会把
//!   类内形变放大到与类间差异同量级，是正确率仅 52.9% 的根因。
//! - 匹配用 NCC + ±1px 位移补偿：样本实测类内 min 0.931 / 跨类 max(3↔8) 0.839，
//!   阈值 [`NCC_MIN`]=0.88 一刀切分离，[`NCC_GAP`] 兜平局。
//!
//! 达不到阈值 → None（上层刷新重试；绝不硬猜——错答案会消耗 CAS 连续错误计数）。

use serde::{Deserialize, Serialize};

/// 字符画布：高 16（数字实际 13 行 + 抖动余量）/ 宽 14（最宽字符 12 列 + 余量）。
pub const CANVAS_H: usize = 16;
pub const CANVAS_W: usize = 14;
const CANVAS_LEN: usize = CANVAS_H * CANVAS_W;

/// 切分用强度阈值（相对本图峰值）：字符主体 ≥0.5，取 0.12 保细笔画与淡彩。
const SEG_MIN: f32 = 0.12;
/// 接受阈值与平局间隙（样本标定：类内 min 0.931 / 跨类 max 0.839）。
const NCC_MIN: f32 = 0.88;
const NCC_GAP: f32 = 0.03;

/// 单条模板：一个字符类 + 一张 16×14 强度画布（0..255，行优先）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    #[serde(rename = "char")]
    pub ch: char,
    pub grid: Vec<u8>,
}

/// 模板集文件结构（templates/kaptcha-templates.json）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateFile {
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

/// 编译期内嵌的模板集（由 `tests/captcha_solve.rs::build_templates` 生成）。
pub struct KaptchaTemplates {
    file: TemplateFile,
}

impl KaptchaTemplates {
    pub fn load() -> Self {
        Self {
            file: serde_json::from_str(include_str!("../../templates/kaptcha-templates.json"))
                .expect("templates/kaptcha-templates.json 为构建脚本产物，格式应恒合法"),
        }
    }

    pub fn templates(&self) -> &[TemplateEntry] {
        &self.file.templates
    }
}

/// PNG 字节 → 算术答案字符串（如 "63"）。
/// None = 无法识别（上层刷新重试，重试穷尽转手动）。
pub fn solve(png_bytes: &[u8], t: &KaptchaTemplates) -> Option<String> {
    let patches = png_to_patches(png_bytes)?;
    solve_patches(&patches, t)
}

/// PNG 字节 → 前 3 片字符的 16×14 强度画布。
/// 正常题面恒 4 片 [d1, op, d2, =]；片数 ≠4 意味着字符断裂（多片）或粘连（少
/// 片），片序已不可信——返回 None 而不是硬匹配（错位片可能「自信地」匹配出错误
/// 答案，消耗 CAS 连续错误计数，比 None 危险）。
pub fn png_to_patches(png_bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let img = image::load_from_memory(png_bytes).ok()?.to_rgb8();
    let (w, h) = img.dimensions();
    let nm = fg_intensity(&img);
    let segs = vertical_segments(&nm, w, h);
    if segs.len() != 4 {
        return None;
    }
    Some(
        segs[..3]
            .iter()
            .map(|&(x0, x1)| patch_at(&nm, w, h, x0, x1))
            .collect(),
    )
}

/// 已切分的 3 片 16×14 画布 → 求值字符串。片序 = [d1, op, d2]（与题面渲染顺序一致）。
pub fn solve_patches(patches: &[Vec<u8>], t: &KaptchaTemplates) -> Option<String> {
    let [p1, p2, p3] = patches else { return None };
    // char → 数值必须 to_digit（as i32 会取 ASCII 码：'8' → 56）
    let d1 = classify(p1, t, is_digit)?.to_digit(10)? as i32;
    let op = classify(p2, t, is_op)?;
    let d2 = classify(p3, t, is_digit)?.to_digit(10)? as i32;
    let val = match op {
        '+' => d1 + d2,
        '-' => d1 - d2,
        '*' => d1 * d2,
        // classify 的 is_op 过滤已保证只有 + - *；此处仅为不放行未知运算符
        _ => return None,
    };
    Some(val.to_string())
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_op(c: char) -> bool {
    matches!(c, '+' | '-' | '*')
}

/// 每类最佳 NCC（降序）。标定/诊断与 [`classify`] 共用同一实现。
pub fn class_scores(patch: &[u8], t: &KaptchaTemplates, filter: fn(char) -> bool) -> Vec<(char, f32)> {
    let mut per_char: Vec<(char, f32)> = Vec::new();
    for e in t.templates() {
        if !(filter)(e.ch) || e.grid.len() != CANVAS_LEN {
            continue;
        }
        let s = shifted_ncc(patch, &e.grid);
        match per_char.iter_mut().find(|(c, _)| *c == e.ch) {
            Some((_, best)) if s > *best => *best = s,
            Some(_) => {}
            None => per_char.push((e.ch, s)),
        }
    }
    per_char.sort_by(|a, b| b.1.total_cmp(&a.1));
    per_char
}

/// 在满足 `filter` 的模板中找 NCC 最大者；阈值/平局双判 None。
/// 同类多模板先聚合为「每类一个最大分」再跨类比较——否则同类模板间 0.95 vs 1.00 的
/// 正常差异会被平局判定误杀（实测导致 100% 拒绝）。
fn classify(patch: &[u8], t: &KaptchaTemplates, filter: fn(char) -> bool) -> Option<char> {
    let per_char = class_scores(patch, t, filter);
    let &(ch, bs) = per_char.first()?;
    if bs < NCC_MIN {
        return None;
    }
    // 次佳为另一字符类的分数：差距不足 NCC_GAP → 区分不可靠 → None
    if let Some(second) = per_char.get(1).map(|x| x.1) {
        if bs - second < NCC_GAP {
            return None;
        }
    }
    Some(ch)
}

/// ±1px 位移补偿后取最大 NCC（渲染确定性高，位移仅为抗一像素级抖动）。
fn shifted_ncc(q: &[u8], tpl: &[u8]) -> f32 {
    let mut best = -1.0f32;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let mut buf = [0u8; CANVAS_LEN];
            for gy in 0..CANVAS_H as i32 {
                for gx in 0..CANVAS_W as i32 {
                    let (sy, sx) = (gy - dy, gx - dx);
                    if (0..CANVAS_H as i32).contains(&sy) && (0..CANVAS_W as i32).contains(&sx) {
                        buf[(gy * CANVAS_W as i32 + gx) as usize] =
                            q[(sy * CANVAS_W as i32 + sx) as usize];
                    }
                }
            }
            best = best.max(ncc(&buf, tpl));
        }
    }
    best
}

/// 归一化互相关（长度不等或任一为常数向量 → -1.0）。
fn ncc(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return -1.0;
    }
    let n = a.len() as f32;
    let ma = a.iter().map(|&v| v as f32).sum::<f32>() / n;
    let mb = b.iter().map(|&v| v as f32).sum::<f32>() / n;
    let (mut num, mut da, mut db) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        let xa = *x as f32 - ma;
        let yb = *y as f32 - mb;
        num += xa * yb;
        da += xa * xa;
        db += yb * yb;
    }
    if da <= 1e-9 || db <= 1e-9 {
        return -1.0;
    }
    num / (da.sqrt() * db.sqrt())
}

/// 颜色不变强度图：`dist = 765 - Σrgb` 与字符颜色成线性比例，除以本图峰值即与颜色无关。
/// 返回与像素数等长的 0..1 强度（背景恒 0）。
fn fg_intensity(rgb: &image::RgbImage) -> Vec<f32> {
    let mut d: Vec<f32> = rgb
        .pixels()
        .map(|p| (765 - p[0] as u32 - p[1] as u32 - p[2] as u32) as f32)
        .collect();
    let max = d.iter().copied().fold(0.0f32, f32::max).max(1.0);
    for v in d.iter_mut() {
        *v /= max;
    }
    d
}

/// 垂直投影切分：按列强度是否超阈值切段，返回 (x0, x1) 列范围（不含 x1）。
fn vertical_segments(nm: &[f32], w: u32, h: u32) -> Vec<(u32, u32)> {
    let col_has_fg = |x: u32| (0..h).any(|y| nm[(y * w + x) as usize] > SEG_MIN);
    let mut segs = Vec::new();
    let mut start: Option<u32> = None;
    for x in 0..w {
        match (start, col_has_fg(x)) {
            (None, true) => start = Some(x),
            (Some(s), false) => {
                segs.push((s, x));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        segs.push((s, w));
    }
    segs
}

/// 片段 → 16×14 强度画布：以片内前景行范围为 bbox，**左上角锚定**（渲染位置确定，
/// 锚定保留真实形状；缩放会引入形变噪声）。
fn patch_at(nm: &[f32], w: u32, h: u32, x0: u32, x1: u32) -> Vec<u8> {
    // 只需片内首个前景行作锚点（渲染位置确定，左上角锚定即保留形状）
    let mut y0 = h;
    for y in 0..h {
        if (x0..x1).any(|x| nm[(y * w + x) as usize] > SEG_MIN) {
            y0 = y;
            break;
        }
    }
    if y0 == h {
        y0 = 0;
    }
    let mut out = vec![0u8; CANVAS_LEN];
    for gy in 0..CANVAS_H {
        let py = y0 as usize + gy;
        if py >= h as usize {
            break;
        }
        for gx in 0..CANVAS_W {
            let px = x0 as usize + gx;
            if px >= w as usize {
                break;
            }
            out[gy * CANVAS_W + gx] = (nm[py * w as usize + px] * 255.0).round() as u8;
        }
    }
    out
}
