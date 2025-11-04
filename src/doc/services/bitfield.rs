use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bitfield 条目描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitfieldEntry {
    pub name: Option<String>,
    pub bits: u32,
    pub r#type: Option<String>,
    #[serde(default)]
    pub attr: Option<serde_json::Value>,
    #[serde(default)]
    pub rotate: Option<f64>,
    #[serde(default)]
    pub overline: Option<bool>,

    // 计算字段（渲染时填充）
    #[serde(skip)]
    pub lsb: u32,
    #[serde(skip)]
    pub msb: u32,
    #[serde(skip)]
    pub lsbm: u32,
    #[serde(skip)]
    pub msbm: u32,
}

/// Bitfield 渲染器配置
#[derive(Debug, Clone)]
pub struct BitfieldRenderer {
    pub vspace: u32,
    pub hspace: u32,
    pub lanes: u32,
    pub bits: Option<u32>,
    pub fontsize: u32,
    pub fontfamily: String,
    pub fontweight: String,
    pub compact: bool,
    pub hflip: bool,
    pub vflip: bool,
    pub stroke_width: f64,
    pub trim_char_width: Option<f64>,
    pub uneven: bool,
    pub legend: Option<HashMap<String, String>>,
}

impl Default for BitfieldRenderer {
    fn default() -> Self {
        Self {
            vspace: 80,
            hspace: 800,
            lanes: 1,
            bits: None,
            fontsize: 14,
            fontfamily: "sans-serif".to_string(),
            fontweight: "normal".to_string(),
            compact: false,
            hflip: false,
            vflip: false,
            stroke_width: 1.0,
            trim_char_width: None,
            uneven: false,
            legend: None,
        }
    }
}

impl BitfieldRenderer {
    pub fn new(options: &crate::cli::handlers::BitfieldOptions) -> Result<Self, String> {
        // 验证参数
        if options.vspace.map_or(false, |v| v <= 19) {
            return Err("vspace must be greater than 19".to_string());
        }
        if options.hspace.map_or(false, |v| v <= 39) {
            return Err("hspace must be greater than 39".to_string());
        }
        if options.lanes.map_or(false, |v| v == 0) {
            return Err("lanes must be greater than 0".to_string());
        }
        if options.bits.map_or(false, |v| v <= 4) {
            return Err("bits must be greater than 4".to_string());
        }
        if options.fontsize <= 5 {
            return Err("fontsize must be greater than 5".to_string());
        }

        Ok(Self {
            vspace: options.vspace.unwrap_or(80),
            hspace: options.hspace.unwrap_or(800),
            lanes: options.lanes.unwrap_or(1),
            bits: options.bits,
            fontsize: options.fontsize,
            fontfamily: options.fontfamily.clone(),
            fontweight: options.fontweight.clone(),
            compact: options.compact,
            hflip: options.hflip,
            vflip: options.vflip,
            stroke_width: options.strokewidth as f64,
            trim_char_width: options.trim.map(|v| v as f64),
            uneven: options.uneven,
            legend: if options.legend.is_empty() {
                None
            } else {
                Some(options.legend.iter().cloned().collect())
            },
        })
    }

    /// 计算总位数
    pub fn get_total_bits(&self, desc: &[BitfieldEntry]) -> u32 {
        desc.iter().map(|e| e.bits).sum()
    }

    /// 渲染 bitfield 为 SVG
    pub fn render(&mut self, desc: &mut [BitfieldEntry]) -> Result<String, String> {
        // 计算总位数
        let total_bits = self.bits.unwrap_or_else(|| self.get_total_bits(desc));
        self.bits = Some(total_bits);

        let mod_bits = (total_bits + self.lanes - 1) / self.lanes;

        // 计算每个条目的 LSB/MSB
        let mut lsb = 0;
        for e in desc.iter_mut() {
            e.lsb = lsb;
            lsb += e.bits;
            e.msb = lsb - 1;
            e.lsbm = e.lsb % mod_bits;
            e.msbm = e.msb % mod_bits;
            if e.r#type.is_none() {
                e.r#type = None;
            }
        }

        // 计算最大属性数量
        let max_attr_count = desc
            .iter()
            .filter_map(|e| {
                e.attr.as_ref().and_then(|a| {
                    if let Some(arr) = a.as_array() {
                        Some(arr.len())
                    } else {
                        Some(1)
                    }
                })
            })
            .max()
            .unwrap_or(0);

        // 计算高度
        let vlane = if !self.compact {
            self.vspace as f64 - (self.fontsize as f64 * (1.2 + max_attr_count as f64))
        } else {
            self.vspace as f64 - (self.fontsize as f64 * 1.2)
        };

        let height = if !self.compact {
            (self.vspace as f64 * self.lanes as f64) + self.stroke_width / 2.0
        } else {
            vlane * (self.lanes as f64 - 1.0) + self.vspace as f64 + self.stroke_width / 2.0
        };

        let height = if self.legend.is_some() {
            height + (self.fontsize as f64 * 1.2)
        } else {
            height
        };

        // 开始构建 SVG
        let mut svg = SvgBuilder::new(self.hspace, height as u32);

        // 添加 legend
        if let Some(ref legend) = self.legend {
            svg.add_legend(legend, self)?;
        }

        // 添加每个 lane
        for i in 0..self.lanes {
            let lane_index = if self.hflip { i } else { self.lanes - i - 1 };
            self.render_lane(&mut svg, desc, i, lane_index, mod_bits, vlane)?;
        }

        Ok(svg.to_string())
    }

    fn render_lane(
        &self,
        svg: &mut SvgBuilder,
        desc: &[BitfieldEntry],
        index: u32,
        lane_index: u32,
        mod_bits: u32,
        vlane: f64,
    ) -> Result<(), String> {
        let dy = if self.compact {
            if index > 0 {
                (index - 1) as f64 * vlane + self.vspace as f64
            } else {
                0.0
            }
        } else {
            index as f64 * self.vspace as f64
        };

        let dy = if self.legend.is_some() {
            dy + (self.fontsize as f64 * 1.2)
        } else {
            dy
        };

        svg.start_group(format!("translate(0, {})", dy));

        // 渲染标签
        self.render_labels(svg, desc, index, lane_index, mod_bits, vlane)?;

        // 渲染框架
        self.render_cage(svg, desc, index, lane_index, mod_bits, vlane)?;

        svg.end_group();

        Ok(())
    }

    fn render_labels(
        &self,
        svg: &mut SvgBuilder,
        desc: &[BitfieldEntry],
        index: u32,
        lane_index: u32,
        mod_bits: u32,
        vlane: f64,
    ) -> Result<(), String> {
        let step = self.hspace as f64 / mod_bits as f64;

        // 外层g：text-anchor="middle"
        svg.start_group("translate(0, 0)".to_string());
        svg.set_text_anchor("middle");

        // bits 组：translate(step/2, fontsize)
        svg.start_group(format!(
            "translate({}, {})",
            step / 2.0,
            self.fontsize as f64
        ));

        // 只在非 compact 或第一个 lane 时显示位号
        if !self.compact || index == 0 {
            for e in desc {
                let mut lsbm = 0;
                let mut msbm = mod_bits - 1;
                let mut lsb = lane_index * mod_bits;
                let mut msb = (lane_index + 1) * mod_bits - 1;

                // 检查entry是否在当前lane中
                if e.lsb / mod_bits == lane_index {
                    lsbm = e.lsbm;
                    lsb = e.lsb;
                    if e.msb / mod_bits == lane_index {
                        msb = e.msb;
                        msbm = e.msbm;
                    }
                } else {
                    if e.msb / mod_bits == lane_index {
                        msb = e.msb;
                        msbm = e.msbm;
                    } else if !(lsb > e.lsb && msb < e.msb) {
                        continue;
                    }
                }

                let msb_pos = if self.vflip {
                    msbm as f64
                } else {
                    (mod_bits - msbm - 1) as f64
                };
                let lsb_pos = if self.vflip {
                    lsbm as f64
                } else {
                    (mod_bits - lsbm - 1) as f64
                };

                if !self.compact {
                    svg.add_text(step * lsb_pos, 0.0, &lsb.to_string(), self);
                    if lsbm != msbm {
                        svg.add_text(step * msb_pos, 0.0, &msb.to_string(), self);
                    }
                }
            }

            // Compact 模式下的位号显示
            if self.compact && index == 0 {
                for i in 0..mod_bits {
                    let bit_num = if self.vflip { i } else { mod_bits - i - 1 };
                    svg.add_text(step * i as f64, 0.0, &bit_num.to_string(), self);
                }
            }
        }

        svg.end_group(); // bits组结束

        // 内层g：translate(0, fontsize*1.2)，包含 blanks, names, attrs
        if !self.compact || index == 0 {
            svg.start_group(format!("translate(0, {})", self.fontsize as f64 * 1.2));

            // blanks 组：背景矩形（在translate(0, 0)中，但实际位置由外层g控制）
            svg.start_group("translate(0, 0)".to_string());
            for e in desc {
                let mut lsbm = 0;
                let mut msbm = mod_bits - 1;
                let lsb = lane_index * mod_bits;
                let msb = (lane_index + 1) * mod_bits - 1;

                if e.lsb / mod_bits == lane_index {
                    lsbm = e.lsbm;
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    }
                } else {
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    } else if !(lsb > e.lsb && msb < e.msb) {
                        continue;
                    }
                }

                let msb_pos = if self.vflip {
                    msbm as f64
                } else {
                    (mod_bits - msbm - 1) as f64
                };
                let lsb_pos = if self.vflip {
                    lsbm as f64
                } else {
                    (mod_bits - lsbm - 1) as f64
                };

                if e.name.is_none() || e.r#type.is_some() {
                    let color = type_color(e.r#type.as_deref());
                    let x = step * (if self.vflip { lsb_pos } else { msb_pos });
                    let width = step * (msbm - lsbm + 1) as f64;
                    let height = vlane - self.stroke_width / 2.0;

                    svg.add_rect(x, self.stroke_width / 2.0, width, height, color);
                }
            }
            svg.end_group(); // blanks组结束

            // names 组：translate(step/2, vlane/2 + fontsize/2)
            svg.start_group(format!(
                "translate({}, {})",
                step / 2.0,
                vlane / 2.0 + self.fontsize as f64 / 2.0
            ));
            for e in desc {
                let mut lsbm = 0;
                let mut msbm = mod_bits - 1;
                let lsb = lane_index * mod_bits;
                let msb = (lane_index + 1) * mod_bits - 1;

                if e.lsb / mod_bits == lane_index {
                    lsbm = e.lsbm;
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    }
                } else {
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    } else if !(lsb > e.lsb && msb < e.msb) {
                        continue;
                    }
                }

                let msb_pos = if self.vflip {
                    msbm as f64
                } else {
                    (mod_bits - msbm - 1) as f64
                };
                let lsb_pos = if self.vflip {
                    lsbm as f64
                } else {
                    (mod_bits - lsbm - 1) as f64
                };

                if let Some(ref name) = e.name {
                    let available_space = step * (msbm - lsbm + 1) as f64;
                    let trimmed_name = self.trim_text(name, available_space);
                    let x = step * (msb_pos + lsb_pos) / 2.0;
                    let y = -6.0;

                    let mut attrs = HashMap::new();
                    attrs.insert("font-size".to_string(), self.fontsize.to_string());
                    attrs.insert("font-family".to_string(), self.fontfamily.clone());
                    attrs.insert("font-weight".to_string(), self.fontweight.clone());
                    attrs.insert("text-anchor".to_string(), "middle".to_string());
                    attrs.insert("y".to_string(), "6".to_string());

                    if let Some(rotate) = e.rotate {
                        attrs.insert("transform".to_string(), format!("rotate({})", rotate));
                    }
                    if e.overline == Some(true) {
                        attrs.insert("text-decoration".to_string(), "overline".to_string());
                    }

                    svg.start_group(format!("translate({}, {})", x, y));
                    svg.add_text_with_attrs(0.0, 0.0, &trimmed_name, &attrs);
                    svg.end_group();
                }
            }
            svg.end_group(); // names组结束

            // attrs 组：translate(step/2, vlane + fontsize)
            if !self.compact {
                svg.start_group(format!(
                    "translate({}, {})",
                    step / 2.0,
                    vlane + self.fontsize as f64
                ));
                for e in desc {
                    let mut lsbm = 0;
                    let mut msbm = mod_bits - 1;
                    let lsb = lane_index * mod_bits;
                    let msb = (lane_index + 1) * mod_bits - 1;

                    if e.lsb / mod_bits == lane_index {
                        lsbm = e.lsbm;
                        if e.msb / mod_bits == lane_index {
                            msbm = e.msbm;
                        }
                    } else {
                        if e.msb / mod_bits == lane_index {
                            msbm = e.msbm;
                        } else if !(lsb > e.lsb && msb < e.msb) {
                            continue;
                        }
                    }

                    let msb_pos = if self.vflip {
                        msbm as f64
                    } else {
                        (mod_bits - msbm - 1) as f64
                    };
                    let lsb_pos = if self.vflip {
                        lsbm as f64
                    } else {
                        (mod_bits - lsbm - 1) as f64
                    };

                    if let Some(ref attr) = e.attr {
                        let attr_list: Vec<serde_json::Value> = if attr.is_array() {
                            attr.as_array().unwrap().iter().cloned().collect()
                        } else {
                            vec![attr.clone()]
                        };

                        for (i, attr_value) in attr_list.iter().enumerate() {
                            svg.start_group(format!(
                                "translate(0, {})",
                                i as f64 * self.fontsize as f64
                            ));

                            if let Some(num) = attr_value.as_u64() {
                                // 整数：显示为二进制
                                let num = num as u32;
                                let lsb_val = e.lsb;
                                let msb_val = e.msb;
                                for biti in 0..=(msb_val - lsb_val) {
                                    let bit_set = (num & (1 << (biti + lsb_val - e.lsb))) != 0;
                                    let bit_text = if bit_set { "1" } else { "0" };
                                    let bit_pos = if self.vflip {
                                        lsb_pos + biti as f64
                                    } else {
                                        lsb_pos - biti as f64
                                    };
                                    svg.add_text(step * bit_pos, 0.0, bit_text, self);
                                }
                            } else if let Some(attr_str) = attr_value.as_str() {
                                // 字符串：居中显示
                                let x = step * (msb_pos + lsb_pos) / 2.0;
                                svg.add_text(x, 0.0, attr_str, self);
                            } else {
                                // 其他类型：转换为字符串显示
                                let attr_str = attr_value.to_string();
                                let x = step * (msb_pos + lsb_pos) / 2.0;
                                svg.add_text(x, 0.0, &attr_str, self);
                            }

                            svg.end_group();
                        }
                    }
                }
                svg.end_group(); // attrs组结束
            }

            svg.end_group(); // 内层g结束
        } else {
            // compact模式且不是第一个lane：只渲染 blanks, names, attrs
            svg.start_group(format!("translate(0, {})", self.fontsize as f64 * 1.2));

            // blanks
            svg.start_group("translate(0, 0)".to_string());
            for e in desc {
                let mut lsbm = 0;
                let mut msbm = mod_bits - 1;
                let lsb = lane_index * mod_bits;
                let msb = (lane_index + 1) * mod_bits - 1;

                if e.lsb / mod_bits == lane_index {
                    lsbm = e.lsbm;
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    }
                } else {
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    } else if !(lsb > e.lsb && msb < e.msb) {
                        continue;
                    }
                }

                let msb_pos = if self.vflip {
                    msbm as f64
                } else {
                    (mod_bits - msbm - 1) as f64
                };
                let lsb_pos = if self.vflip {
                    lsbm as f64
                } else {
                    (mod_bits - lsbm - 1) as f64
                };

                if e.name.is_none() || e.r#type.is_some() {
                    let color = type_color(e.r#type.as_deref());
                    let x = step * (if self.vflip { lsb_pos } else { msb_pos });
                    let width = step * (msbm - lsbm + 1) as f64;
                    let height = vlane - self.stroke_width / 2.0;
                    svg.add_rect(x, self.stroke_width / 2.0, width, height, color);
                }
            }
            svg.end_group();

            // names
            svg.start_group(format!(
                "translate({}, {})",
                step / 2.0,
                vlane / 2.0 + self.fontsize as f64 / 2.0
            ));
            for e in desc {
                let mut lsbm = 0;
                let mut msbm = mod_bits - 1;
                let lsb = lane_index * mod_bits;
                let msb = (lane_index + 1) * mod_bits - 1;

                if e.lsb / mod_bits == lane_index {
                    lsbm = e.lsbm;
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    }
                } else {
                    if e.msb / mod_bits == lane_index {
                        msbm = e.msbm;
                    } else if !(lsb > e.lsb && msb < e.msb) {
                        continue;
                    }
                }

                let msb_pos = if self.vflip {
                    msbm as f64
                } else {
                    (mod_bits - msbm - 1) as f64
                };
                let lsb_pos = if self.vflip {
                    lsbm as f64
                } else {
                    (mod_bits - lsbm - 1) as f64
                };

                if let Some(ref name) = e.name {
                    let available_space = step * (msbm - lsbm + 1) as f64;
                    let trimmed_name = self.trim_text(name, available_space);
                    let x = step * (msb_pos + lsb_pos) / 2.0;
                    let y = -6.0;

                    let mut attrs = HashMap::new();
                    attrs.insert("font-size".to_string(), self.fontsize.to_string());
                    attrs.insert("font-family".to_string(), self.fontfamily.clone());
                    attrs.insert("font-weight".to_string(), self.fontweight.clone());
                    attrs.insert("text-anchor".to_string(), "middle".to_string());
                    attrs.insert("y".to_string(), "6".to_string());

                    if let Some(rotate) = e.rotate {
                        attrs.insert("transform".to_string(), format!("rotate({})", rotate));
                    }
                    if e.overline == Some(true) {
                        attrs.insert("text-decoration".to_string(), "overline".to_string());
                    }

                    svg.start_group(format!("translate({}, {})", x, y));
                    svg.add_text_with_attrs(0.0, 0.0, &trimmed_name, &attrs);
                    svg.end_group();
                }
            }
            svg.end_group();

            svg.end_group();
        }

        svg.end_group(); // 外层g结束

        Ok(())
    }

    fn render_cage(
        &self,
        svg: &mut SvgBuilder,
        desc: &[BitfieldEntry],
        index: u32,
        lane_index: u32,
        mod_bits: u32,
        vlane: f64,
    ) -> Result<(), String> {
        let dy = if !self.compact || index == 0 {
            self.fontsize as f64 * 1.2
        } else {
            0.0
        };

        svg.start_group(format!("translate(0, {})", dy));
        svg.set_stroke("black");
        svg.set_stroke_width(self.stroke_width);
        svg.set_stroke_linecap("butt");

        let skip_count = if self.uneven && self.lanes > 1 && lane_index == self.lanes - 1 {
            let skip = mod_bits - (self.bits.unwrap_or(0) % mod_bits);
            if skip == mod_bits {
                0
            } else {
                skip
            }
        } else {
            0
        };

        let hlen = (self.hspace as f64 / mod_bits as f64) * (mod_bits - skip_count) as f64;
        let hpos = if self.vflip {
            0.0
        } else {
            (self.hspace as f64 / mod_bits as f64) * skip_count as f64
        };

        // 绘制水平线
        if !self.compact || self.hflip || lane_index == 0 {
            svg.add_line(hpos, vlane, hpos + hlen, vlane); // bottom
        }
        if !self.compact || !self.hflip || lane_index == 0 {
            svg.add_line(hpos, 0.0, hpos + hlen, 0.0); // top
        }

        // 绘制垂直线
        let hbit = (self.hspace as f64 - self.stroke_width) / mod_bits as f64;
        for bit_pos in 0..mod_bits {
            let bitm = if self.vflip {
                bit_pos
            } else {
                mod_bits - bit_pos - 1
            };
            let bit = lane_index * mod_bits + bitm;
            if bit >= self.bits.unwrap_or(0) {
                continue;
            }

            let rpos = if self.vflip { bit_pos + 1 } else { bit_pos };
            let lpos = if self.vflip { bit_pos } else { bit_pos + 1 };

            if bitm + 1 == mod_bits - skip_count {
                let x = rpos as f64 * hbit + self.stroke_width / 2.0;
                svg.add_line(x, 0.0, x, vlane);
            }
            if bitm == 0 {
                let x = lpos as f64 * hbit + self.stroke_width / 2.0;
                svg.add_line(x, 0.0, x, vlane);
            } else if desc.iter().any(|e| e.lsb == bit) {
                let x = lpos as f64 * hbit + self.stroke_width / 2.0;
                svg.add_line(x, 0.0, x, vlane);
            } else {
                let x = lpos as f64 * hbit + self.stroke_width / 2.0;
                svg.add_line(x, 0.0, x, vlane / 8.0);
                svg.add_line(x, vlane * 7.0 / 8.0, x, vlane);
            }
        }

        svg.end_group();

        Ok(())
    }

    fn trim_text(&self, text: &str, available_space: f64) -> String {
        if let Some(char_width) = self.trim_char_width {
            let text_width = text.len() as f64 * char_width;
            if text_width <= available_space {
                text.to_string()
            } else {
                let end = text.len() - ((text_width - available_space) / char_width) as usize - 3;
                if end > 0 {
                    format!("{}...", &text[..end])
                } else {
                    format!("{}...", &text[..1])
                }
            }
        } else {
            text.to_string()
        }
    }
}

/// SVG 构建器
struct SvgBuilder {
    width: u32,
    height: u32,
    content: String,
    indent: usize,
    text_anchor: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<f64>,
    stroke_linecap: Option<String>,
}

impl SvgBuilder {
    fn new(width: u32, height: u32) -> Self {
        let mut svg = Self {
            width,
            height,
            content: String::new(),
            indent: 0,
            text_anchor: None,
            stroke: None,
            stroke_width: None,
            stroke_linecap: None,
        };

        svg.content
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        svg.content.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            width, height, width, height
        ));
        svg.indent = 1;

        svg
    }

    fn indent_str(&self) -> String {
        "  ".repeat(self.indent)
    }

    fn start_group(&mut self, transform: String) {
        self.content.push_str(&format!(
            "{}<g transform=\"{}\">\n",
            self.indent_str(),
            transform
        ));
        self.indent += 1;
    }

    fn end_group(&mut self) {
        self.indent -= 1;
        self.content
            .push_str(&format!("{}</g>\n", self.indent_str()));
    }

    fn set_text_anchor(&mut self, anchor: &str) {
        self.text_anchor = Some(anchor.to_string());
    }

    fn set_stroke(&mut self, color: &str) {
        self.stroke = Some(color.to_string());
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = Some(width);
    }

    fn set_stroke_linecap(&mut self, linecap: &str) {
        self.stroke_linecap = Some(linecap.to_string());
    }

    fn add_text(&mut self, x: f64, y: f64, text: &str, renderer: &BitfieldRenderer) {
        let mut attrs = HashMap::new();
        attrs.insert("x".to_string(), x.to_string());
        attrs.insert("y".to_string(), y.to_string());
        attrs.insert("font-size".to_string(), renderer.fontsize.to_string());
        attrs.insert("font-family".to_string(), renderer.fontfamily.clone());
        attrs.insert("font-weight".to_string(), renderer.fontweight.clone());

        if let Some(ref anchor) = self.text_anchor {
            attrs.insert("text-anchor".to_string(), anchor.clone());
        }

        self.add_text_with_attrs(x, y, text, &attrs);
    }

    fn add_text_with_attrs(
        &mut self,
        _x: f64,
        _y: f64,
        text: &str,
        attrs: &HashMap<String, String>,
    ) {
        let attr_str = attrs
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect::<Vec<_>>()
            .join(" ");

        // 处理 tspan（简单的文本格式化）
        let formatted_text = format_tspan(text);

        self.content.push_str(&format!(
            "{}<text {}>{}</text>\n",
            self.indent_str(),
            attr_str,
            formatted_text
        ));
    }

    fn add_rect(&mut self, x: f64, y: f64, width: f64, height: f64, fill: String) {
        self.content.push_str(&format!(
            "{}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            self.indent_str(),
            x,
            y,
            width,
            height,
            fill
        ));
    }

    fn add_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let mut attrs = vec![
            format!("x1=\"{}\"", x1),
            format!("y1=\"{}\"", y1),
            format!("x2=\"{}\"", x2),
            format!("y2=\"{}\"", y2),
        ];

        if let Some(ref stroke) = self.stroke {
            attrs.push(format!("stroke=\"{}\"", stroke));
        }
        if let Some(stroke_width) = self.stroke_width {
            attrs.push(format!("stroke-width=\"{}\"", stroke_width));
        }
        if let Some(ref linecap) = self.stroke_linecap {
            attrs.push(format!("stroke-linecap=\"{}\"", linecap));
        }

        self.content.push_str(&format!(
            "{}<line {}/>\n",
            self.indent_str(),
            attrs.join(" ")
        ));
    }

    fn add_legend(
        &mut self,
        legend: &HashMap<String, String>,
        renderer: &BitfieldRenderer,
    ) -> Result<(), String> {
        self.start_group(format!("translate(0, {})", renderer.stroke_width / 2.0));

        let name_padding = 64.0;
        let square_padding = 20.0;
        let legend_count = legend.len() as f64;
        let mut x =
            renderer.hspace as f64 / 2.0 - legend_count / 2.0 * (square_padding + name_padding);

        for (key, value) in legend {
            let color = type_color(Some(value));
            self.add_rect(x, 0.0, 12.0, 12.0, color.clone());
            x += square_padding;
            self.add_text(x, renderer.fontsize as f64 / 1.2, key, renderer);
            x += name_padding;
        }

        self.end_group();
        Ok(())
    }

    fn to_string(mut self) -> String {
        self.content.push_str("</svg>\n");
        self.content
    }
}

/// 将 HLS 转换为 RGB
fn hls_to_rgb(h: f64, l: f64, s: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0).min(255.0).max(0.0) as u8,
        ((g + m) * 255.0).min(255.0).max(0.0) as u8,
        ((b + m) * 255.0).min(255.0).max(0.0) as u8,
    )
}

/// 获取类型颜色
fn type_color(t: Option<&str>) -> String {
    let styles: HashMap<&str, f64> = [
        ("2", 0.0),
        ("3", 80.0),
        ("4", 170.0),
        ("5", 45.0),
        ("6", 126.0),
        ("7", 215.0),
    ]
    .iter()
    .cloned()
    .collect();

    if let Some(t) = t {
        if let Some(&hue) = styles.get(t) {
            let (r, g, b) = hls_to_rgb(hue, 0.9, 1.0);
            format!("rgb({}, {}, {})", r, g, b)
        } else {
            "rgb(229, 229, 229)".to_string()
        }
    } else {
        "rgb(229, 229, 229)".to_string()
    }
}

/// 格式化 tspan 文本（支持简单的 HTML 标签）
fn format_tspan(text: &str) -> String {
    // 简单的文本格式化实现
    // 支持 <b>, <i>, <u>, <s>, <sub>, <sup>, <tt>, <o> 等标签
    let mut result = text.to_string();

    // 替换标签为 SVG tspan
    result = result.replace("<b>", "<tspan font-weight=\"bold\">");
    result = result.replace("</b>", "</tspan>");
    result = result.replace("<i>", "<tspan font-style=\"italic\">");
    result = result.replace("</i>", "</tspan>");
    result = result.replace("<u>", "<tspan text-decoration=\"underline\">");
    result = result.replace("</u>", "</tspan>");
    result = result.replace("<s>", "<tspan text-decoration=\"line-through\">");
    result = result.replace("</s>", "</tspan>");
    result = result.replace("<o>", "<tspan text-decoration=\"overline\">");
    result = result.replace("</o>", "</tspan>");
    result = result.replace(
        "<sub>",
        "<tspan baseline-shift=\"sub\" font-size=\"0.7em\">",
    );
    result = result.replace("</sub>", "</tspan>");
    result = result.replace(
        "<sup>",
        "<tspan baseline-shift=\"super\" font-size=\"0.7em\">",
    );
    result = result.replace("</sup>", "</tspan>");
    result = result.replace("<tt>", "<tspan font-family=\"monospace\">");
    result = result.replace("</tt>", "</tspan>");

    result
}

/// 美化 SVG XML（简单的格式化）
fn beautify_svg(svg: &str) -> String {
    // 简单的美化：添加换行和缩进
    // 使用更简单的方法：直接在现有SVG基础上格式化
    let mut result = String::new();
    let mut indent: usize = 0;
    let lines: Vec<&str> = svg.lines().collect();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("</") {
            indent = indent.saturating_sub(1);
            result.push_str(&"  ".repeat(indent));
            result.push_str(trimmed);
            result.push('\n');
        } else if trimmed.ends_with("/>")
            || trimmed.starts_with("<?")
            || trimmed.starts_with("<!--")
        {
            result.push_str(&"  ".repeat(indent));
            result.push_str(trimmed);
            result.push('\n');
        } else if trimmed.starts_with("<") && !trimmed.starts_with("</") {
            result.push_str(&"  ".repeat(indent));
            result.push_str(trimmed);
            result.push('\n');
            if !trimmed.ends_with("/>") {
                indent += 1;
            }
        } else {
            result.push_str(&"  ".repeat(indent));
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    result
}

/// 从 JSON 文件渲染 bitfield
pub fn render_bitfield_from_json(
    json_path: &std::path::Path,
    options: &crate::cli::handlers::BitfieldOptions,
) -> Result<String, String> {
    use crate::utils::fs;
    let json_content =
        fs::read_to_string(json_path).map_err(|e| format!("Failed to read JSON file: {}", e))?;

    // 尝试解析 JSON5 或普通 JSON
    let entries: Vec<BitfieldEntry> = if options.json5 {
        // 如果要求使用 json5，但 Rust 没有原生支持，先尝试普通 JSON
        serde_json::from_str(&json_content).map_err(|e| format!("Failed to parse JSON: {}", e))?
    } else if options.no_json5 {
        serde_json::from_str(&json_content).map_err(|e| format!("Failed to parse JSON: {}", e))?
    } else {
        // 默认尝试普通 JSON，如果失败可以考虑提示用户
        serde_json::from_str(&json_content)
            .map_err(|e| format!("Failed to parse JSON: {}. Note: JSON5 support requires json5 crate or manual parsing.", e))?
    };

    let mut renderer = BitfieldRenderer::new(options)?;
    let mut entries = entries;
    let mut svg = renderer.render(&mut entries)?;

    // 如果启用了 beautify，格式化 SVG
    if options.beautify {
        svg = beautify_svg(&svg);
    }

    Ok(svg)
}
