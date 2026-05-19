use std::sync::Arc;

use anyhow::{Result, anyhow};
use dashmap::DashMap;

use crate::terminal::emulator::CellPixels;
use crate::terminal::{Emulator, TerminalId};

pub struct Registry {
    inner: DashMap<TerminalId, Arc<Emulator>>,
}

impl Registry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: DashMap::new(),
        })
    }

    pub fn create(
        &self,
        rows: u16,
        cols: u16,
        shell: Option<String>,
        cell: CellPixels,
    ) -> Result<(TerminalId, Arc<Emulator>)> {
        let emulator = Emulator::spawn(rows, cols, shell, cell)?;
        let id = self.fresh_id();
        self.inner.insert(id.clone(), Arc::clone(&emulator));
        Ok((id, emulator))
    }

    pub fn get(&self, id: &str) -> Result<Arc<Emulator>> {
        self.inner
            .get(id)
            .map(|e| Arc::clone(e.value()))
            .ok_or_else(|| anyhow!("terminal '{id}' not found"))
    }

    pub fn destroy(&self, id: &str) -> Result<()> {
        let emulator = self
            .inner
            .remove(id)
            .ok_or_else(|| anyhow!("terminal '{id}' not found"))?
            .1;
        emulator.kill().ok();
        Ok(())
    }

    pub fn list(&self) -> Vec<(TerminalId, Arc<Emulator>)> {
        self.inner
            .iter()
            .map(|e| (e.key().clone(), Arc::clone(e.value())))
            .collect()
    }

    fn fresh_id(&self) -> TerminalId {
        let alphabet: &[char] = &[
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
            'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7',
            '8', '9',
        ];
        loop {
            let id = nanoid::nanoid!(2, alphabet);
            if !self.inner.contains_key(&id) {
                return id;
            }
        }
    }
}
