//! Образы памяти: ROM команд и RAM данных, по 1024 слова.

use std::collections::BTreeMap;

/// Размер ROM и RAM в словах.
pub const WORDS: u32 = 1024;

/// Слова с адресами. Адреса, в которые ничего не записано, в образе отсутствуют
/// и при загрузке остаются нулями.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Memory {
    words: BTreeMap<u32, u32>,
}

impl Memory {
    /// Записывает слово. Повторная запись по тому же адресу заменяет прежнее слово.
    pub fn set(&mut self, addr: u32, word: u32) {
        self.words.insert(addr, word);
    }

    pub fn get(&self, addr: u32) -> Option<u32> {
        self.words.get(&addr).copied()
    }

    /// Число записанных слов.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Текст для `$readmemh`: слова по одному в строке, `@адрес` в начале и после каждого разрыва.
    pub fn to_hex(&self) -> String {
        let mut text = String::new();
        let mut next = None;
        for (&addr, &word) in &self.words {
            if next != Some(addr) {
                text.push_str(&format!("@{addr:03X}\n"));
            }
            text.push_str(&format!("{word:08X}\n"));
            next = Some(addr + 1);
        }
        text
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    pub rom: Memory,
    pub ram: Memory,
}
