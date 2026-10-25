//! 验证码识别：合成图 roundtrip（离线）+ 模板生成 + 三分类评测（依赖样本，#[ignore]）。
//!
//! - `solve_synthetic_roundtrip`：程序合成彩色验证码 PNG，验证全链路（切分/强度归一化/
//!   NCC/求值），无需网络与样本。
//! - `build_templates`：读 `../../scripts/captcha-samples` + labels.json（70/30 划分，
//!   每类保留最多 `PER_CLASS_LIMIT` 张样本补丁）写 `templates/kaptcha-templates.json`。
//! - `captcha_solve_eval`：同一划分下报告 正确/自信错误/拒绝 三分类（门槛 正确率 ≥98%
//!   且 自信错误 =0——自信错误会消耗 CAS 连续错误计数，是红线）。
//!
//! cargo test 的工作目录 = crate 根（campus-auth），故样本相对路径为 ../../scripts/。

use campus_login_lib::vpn::captcha::{
    class_scores, png_to_patches, solve, solve_patches, KaptchaTemplates, TemplateEntry, TemplateFile,
    CANVAS_H, CANVAS_W,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const SAMPLES_DIR: &str = "tests/captcha-samples";
const TEMPLATES_PATH: &str = "templates/kaptcha-templates.json";
const PER_CLASS_LIMIT: usize = 10;

/// 列出有标注的样本（文件名升序），label 归一（×/x/X → *）并校验 `d op d` 形态。
/// 返回 (文件名, 数字1, 运算符, 数字2)。
fn list_labeled(samples_dir: &Path) -> Vec<(String, u8, char, u8)> {
    let labels_path = samples_dir.join("labels.json");
    let raw: HashMap<String, String> =
        serde_json::from_str(&fs::read_to_string(&labels_path).unwrap_or_default())
            .unwrap_or_default();
    let mut out = Vec::new();
    for (file, label) in raw {
        let ch: Vec<char> = label
            .chars()
            .map(|c| match c {
                '×' | 'x' | 'X' => '*',
                c => c,
            })
            .collect();
        if ch.len() == 3
            && ch[0].is_ascii_digit()
            && matches!(ch[1], '+' | '-' | '*')
            && ch[2].is_ascii_digit()
        {
            out.push((
                file,
                ch[0].to_digit(10).unwrap() as u8,
                ch[1],
                ch[2].to_digit(10).unwrap() as u8,
            ));
        } else {
            println!("[跳过] label 非法: {file} = {label:?}");
        }
    }
    out.sort();
    out
}

/// 样本序号按 70/30 划分：前 `n*7/10` 进模板集，其余 holdout。
fn split_at(total: usize) -> usize {
    total * 7 / 10
}

fn eval_label(a: u8, op: char, b: u8) -> i32 {
    match op {
        '+' => a as i32 + b as i32,
        '-' => a as i32 - b as i32,
        _ => a as i32 * b as i32,
    }
}

// ---------- 离线测试：合成图 roundtrip ----------

/// 合成一片彩色「字符」：用不同内部图案区分（实心 / 十字 / 右半 / 双横杠）。
/// 颜色各异以覆盖「颜色随机 → 强度归一化」路径。
fn paint_shape(img: &mut image::RgbImage, x0: u32, kind: char, color: [u8; 3]) {
    let (top, bottom) = (5u32, 18u32);
    for y in top..=bottom {
        for x in x0..x0 + 8 {
            let on = match kind {
                '8' => true,                                 // 实心
                '+' => y == 11 || y == 12 || (x >= x0 + 3 && x <= x0 + 4), // 十字
                '3' => x >= x0 + 4,                          // 右半
                '=' => y == 10 || y == 15,                   // 双横杠
                _ => false,
            };
            if on {
                img.put_pixel(x, y, image::Rgb(color));
            }
        }
    }
}

/// 合成「d1 op d2 =」四片彩色验证码 PNG（片宽 8、片间距 16，与真实版式同构）。
fn render_png(colors: [[u8; 3]; 4]) -> Vec<u8> {
    let mut img = image::RgbImage::from_pixel(100, 25, image::Rgb([255, 255, 255]));
    paint_shape(&mut img, 2, '8', colors[0]);
    paint_shape(&mut img, 26, '+', colors[1]);
    paint_shape(&mut img, 51, '3', colors[2]);
    paint_shape(&mut img, 77, '=', colors[3]);
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    buf.into_inner()
}

fn templates_from(entries: Vec<(char, Vec<u8>)>) -> KaptchaTemplates {
    KaptchaTemplates::from_json_str(
        &serde_json::to_string(&TemplateFile {
            templates: entries
                .into_iter()
                .map(|(ch, grid)| TemplateEntry { ch, grid })
                .collect(),
        })
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn solve_synthetic_roundtrip() {
    // 同形状不同颜色 → 强度归一化后应识别一致（颜色不变性）
    let warm = render_png([
        [220, 60, 60],
        [60, 200, 90],
        [60, 90, 230],
        [180, 80, 200],
    ]);
    let cool = render_png([
        [60, 60, 220],
        [90, 200, 60],
        [230, 90, 60],
        [200, 80, 180],
    ]);
    let patches = png_to_patches(&warm).expect("合成图应切出 4 片");
    assert_eq!(patches.len(), 3);
    assert_eq!(patches[0].len(), CANVAS_H * CANVAS_W);
    let t = templates_from(vec![
        ('8', patches[0].clone()),
        ('+', patches[1].clone()),
        ('3', patches[2].clone()),
    ]);
    // 8 + 3 = 11
    assert_eq!(solve(&warm, &t).as_deref(), Some("11"));
    // 换一组颜色（峰值不同、色相不同）→ 仍为 11
    assert_eq!(solve(&cool, &t).as_deref(), Some("11"));

    // 模板与图不符（棋盘噪声）→ 低于 NCC 阈值 → None（不硬猜）
    let noise: Vec<u8> = (0..CANVAS_H * CANVAS_W)
        .map(|i| if i % 3 == 0 { 255 } else { 0 })
        .collect();
    let t_bad = templates_from(vec![
        ('8', noise.clone()),
        ('+', noise.clone()),
        ('3', noise.clone()),
    ]);
    assert_eq!(solve(&warm, &t_bad), None);

    // 片数不足 4（合成 3 片）→ None
    let mut img = image::RgbImage::from_pixel(100, 25, image::Rgb([255, 255, 255]));
    paint_shape(&mut img, 2, '8', [220, 60, 60]);
    paint_shape(&mut img, 26, '+', [60, 200, 90]);
    paint_shape(&mut img, 51, '3', [60, 90, 230]);
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    assert_eq!(png_to_patches(&buf.into_inner()), None);

    // solve_patches 直喂（上层复用入口）：运算符位喂数字补丁 → None
    assert_eq!(solve_patches(&patches, &t_bad), None);
}

// ---------- 依赖样本的测试（#[ignore]，主智能体运行） ----------

/// 样本 + labels → 切分 → 每类留最多 PER_CLASS_LIMIT 张样本补丁作模板 → 写 json。
/// labels.json / 样本目录缺失时打印说明并跳过（不 panic，等标注就位后重跑）。
#[test]
#[ignore]
fn build_templates() {
    let samples_dir = Path::new(SAMPLES_DIR);
    if !samples_dir.join("labels.json").exists() {
        println!(
            "[跳过] {SAMPLES_DIR}/labels.json 不存在（标注分身产出后就位），本测试不生成模板。"
        );
        return;
    }
    let labeled = list_labeled(samples_dir);
    if labeled.is_empty() {
        println!("[跳过] labels.json 无有效条目。");
        return;
    }

    let split = split_at(labeled.len());
    let (train, holdout) = labeled.split_at(split);

    // 每类保留最多 PER_CLASS_LIMIT 张样本补丁作模板。
    // 不用「类内均值」：样本间存在 1px 级渲染抖动，均值会把同类 NCC 从 ~0.99 拉到阈值
    // 边缘（实测正确率从 ~100% 跌到 54%），多模板取最大值才是该目标的正确策略。
    let mut per_class: HashMap<char, usize> = HashMap::new();
    let mut templates: Vec<TemplateEntry> = Vec::new();
    let mut train_fail = 0usize;
    for (file, a, op, b) in train {
        let patches = match fs::read(samples_dir.join(file))
            .map_err(|e| e.to_string())
            .and_then(|png| png_to_patches(&png).ok_or_else(|| "片数≠4".to_string()))
        {
            Ok(p) => p,
            Err(e) => {
                println!("[跳过] {file}: {e}");
                train_fail += 1;
                continue;
            }
        };
        // 片序 = [d1, op, d2]
        let chars = [char::from(b'0' + *a), *op, char::from(b'0' + *b)];
        for (ch, patch) in chars.into_iter().zip(patches) {
            let used = per_class.entry(ch).or_insert(0);
            if *used < PER_CLASS_LIMIT {
                templates.push(TemplateEntry { ch, grid: patch });
                *used += 1;
            }
        }
    }

    // 类覆盖统计（13 类：0-9 + - *）
    let mut missing = Vec::new();
    for ch in "0123456789+-*".chars() {
        let n = templates.iter().filter(|t| t.ch == ch).count();
        println!("类 {ch}: {n} 张模板");
        if n == 0 {
            missing.push(ch);
        }
    }
    if !missing.is_empty() {
        println!("[警告] 缺样本类: {missing:?}（该类字符识别将返回 None，上层兜底）");
    }
    if train_fail > 0 {
        println!("[警告] 模板集侧切分失败 {train_fail} 个样本");
    }

    fs::create_dir_all("templates").unwrap();
    let template_count = templates.len();
    fs::write(
        TEMPLATES_PATH,
        serde_json::to_string_pretty(&TemplateFile { templates }).unwrap(),
    )
    .unwrap();
    println!(
        "已写 {TEMPLATES_PATH}：{template_count} 条模板（模板集 {} 样本 / holdout {} 样本）",
        train.len(),
        holdout.len()
    );
}

/// 评测：同一 70/30 划分下，报告 正确 / 自信错误 / 拒绝 三分类。
/// 模板以磁盘上的 kaptcha-templates.json 为准（先跑 build_templates）。
/// wrong=自信错误（提交后吃 CAS 连续错误计数，红线）；None=拒绝（刷新重试，无害）。
#[test]
fn captcha_solve_eval() {
    let samples_dir = Path::new(SAMPLES_DIR);
    let templates_path = Path::new(TEMPLATES_PATH);
    if !templates_path.exists() {
        println!("[跳过] {TEMPLATES_PATH} 不存在，先运行 build_templates 生成模板。");
        return;
    }
    if !samples_dir.join("labels.json").exists() {
        println!("[跳过] {SAMPLES_DIR}/labels.json 不存在，无法评测。");
        return;
    }
    let t = KaptchaTemplates::from_json_str(&fs::read_to_string(templates_path).unwrap()).unwrap();
    let labeled = list_labeled(samples_dir);
    let split = split_at(labeled.len());

    let acc = |rows: &[(String, u8, char, u8)]| -> (usize, usize, usize) {
        let mut hit = 0;
        let mut wrong = 0;
        let mut total = 0;
        for (file, a, op, b) in rows {
            let Ok(png) = fs::read(samples_dir.join(file)) else {
                continue;
            };
            total += 1;
            let expected = eval_label(*a, *op, *b).to_string();
            match solve(&png, &t) {
                Some(ans) if ans == expected => hit += 1,
                Some(_) => wrong += 1,
                None => {
                    // 诊断：区分切分层拒绝与匹配层拒绝，并打出每片 Top3 分数
                    match png_to_patches(&png) {
                        None => println!("[切分拒绝] {file}"),
                        Some(patches) => {
                            let filters: [fn(char) -> bool; 3] = [
                                |c: char| c.is_ascii_digit(),
                                |c: char| matches!(c, '+' | '-' | '*'),
                                |c: char| c.is_ascii_digit(),
                            ];
                            let expect_chars = [char::from(b'0' + *a), *op, char::from(b'0' + *b)];
                            let detail: Vec<String> = patches
                                .iter()
                                .zip(&filters)
                                .zip(&expect_chars)
                                .map(|((p, f), want)| {
                                    let s = class_scores(p, &t, *f);
                                    let top: Vec<String> = s
                                        .iter()
                                        .take(3)
                                        .map(|(c, v)| format!("{c}:{v:.3}"))
                                        .collect();
                                    format!("{want}=[{}]", top.join(" "))
                                })
                                .collect();
                            println!("[匹配拒绝] {file} {}", detail.join("  "));
                        }
                    }
                }
            }
        }
        (hit, wrong, total)
    };

    let (train, holdout) = labeled.split_at(split);
    let (th, tw, tt) = acc(train);
    let (hh, hw, ht) = acc(holdout);
    let pct = |h: usize, n: usize| {
        if n > 0 {
            format!("{:.1}%", h as f64 / n as f64 * 100.0)
        } else {
            "n/a".to_string()
        }
    };
    println!(
        "模板集: 正确 {th}/{tt} ({})  自信错误 {tw} ({})  拒绝 {} ({})",
        pct(th, tt),
        pct(tw, tt),
        tt - th - tw,
        pct(tt - th - tw, tt)
    );
    println!(
        "holdout: 正确 {hh}/{ht} ({})  自信错误 {hw} ({})  拒绝 {} ({})",
        pct(hh, ht),
        pct(hw, ht),
        ht - hh - hw,
        pct(ht - hh - hw, ht)
    );
    assert!(
        tt == 0 || th * 100 >= tt * 98,
        "模板集正确率 < 98%（样本实测该目标应达 ~100%，回落说明渲染特性变化或模板失效）"
    );
    assert!(
        ht == 0 || hh * 100 >= ht * 98,
        "holdout 正确率 < 98%"
    );
    assert_eq!(tw, 0, "模板集出现自信错误（红线：错答案会消耗 CAS 连续错误计数）");
    assert_eq!(hw, 0, "holdout 出现自信错误（红线）");
}
