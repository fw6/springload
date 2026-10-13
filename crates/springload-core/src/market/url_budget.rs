//! 批量请求的 URL 长度切分。
//!
//! 三家行情接口的 URL 上限都在 8 KB 附近，腾讯在 HTTP/2 下更严。把一批代码切成
//! 若干批时同时看两个约束：单批 URL 长度不超过上限，单批代码数不超过接口的硬上限。

/// 一种数据源的批量约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// URL 里除代码列表之外的部分长度（含等号与查询参数）。
    pub base_len: usize,
    /// 每个代码拼进 URL 时额外增加的字符数（市场前缀与分隔点）。
    pub prefix_len: usize,
    /// 整条 URL 允许的最大字符数。
    pub max_url_len: usize,
    /// 单批允许的最大代码数。
    pub max_codes: usize,
}

impl Budget {
    /// 一批代码拼出来的 URL 长度。
    pub fn url_len(&self, batch: &[String]) -> usize {
        if batch.is_empty() {
            return self.base_len;
        }
        let codes = batch
            .iter()
            .map(|code| code.len() + self.prefix_len)
            .sum::<usize>();
        self.base_len + codes + batch.len() - 1
    }

    /// 把代码切成若干批，逐批满足长度与代码数约束。
    ///
    /// 单个代码本身就让 URL 超长时仍会单独成批，此时已经无法再切。
    pub fn batches(&self, codes: &[String]) -> Vec<Vec<String>> {
        let mut batches: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        let mut current_len = self.base_len;
        for code in codes {
            let cost = code.len() + self.prefix_len;
            let extra = if current.is_empty() { cost } else { cost + 1 };
            let too_long = current_len + extra > self.max_url_len;
            let too_many = current.len() >= self.max_codes;
            if !current.is_empty() && (too_long || too_many) {
                batches.push(std::mem::take(&mut current));
                current_len = self.base_len;
            }
            if current.is_empty() {
                current_len += cost;
            } else {
                current_len += cost + 1;
            }
            current.push(code.clone());
        }
        if !current.is_empty() {
            batches.push(current);
        }
        batches
    }
}
