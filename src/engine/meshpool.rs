//! **Meshing on worker threads.** Turning a chunk's blocks into triangles
//! takes a few milliseconds, which used to come out of the frame. Now the
//! game hands a copy of the chunk and its neighbours to a few worker
//! threads, and picks up the finished meshes on later frames.
//!
//! - Each job carries a number; a chunk changed again while its mesh was
//!   being made gets a fresh job, and the older mesh is thrown away when it
//!   comes back (so what's drawn is never older than what's there).
//! - The few chunks right around the player that were just edited are still
//!   meshed on the spot, so a broken block vanishes on the same frame.
//! - Screenshots and tests mesh everything on the spot, as before.

use crate::mesher::ChunkMesh;
use crate::tint::Field;
use crate::world::{Chunk, World};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

type Key = (i32, i32);

struct Job {
    key: Key,
    seq: u64,
    hood: Box<[Option<Chunk>; 9]>,
    field: Option<Field>,
}

pub struct MeshPool {
    tx: Sender<Job>,
    rx: Receiver<(Key, u64, ChunkMesh)>,
    /// The newest job for each chunk still out.
    latest: HashMap<Key, u64>,
    next: u64,
    /// Jobs sent and not back yet.
    out: usize,
    workers: usize,
}

impl MeshPool {
    /// A pool of `workers` threads (at least one).
    pub fn new(workers: usize) -> MeshPool {
        let workers = workers.max(1);
        let (tx, job_rx) = channel::<Job>();
        let (done_tx, rx) = channel();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for _ in 0..workers {
            let (jobs, done) = (job_rx.clone(), done_tx.clone());
            let _ = std::thread::Builder::new().name("mesher".into()).spawn(move || loop {
                let job = jobs.lock().ok().and_then(|r| r.recv().ok());
                let Some(job) = job else { return };
                let hood = std::array::from_fn(|i| job.hood[i].as_ref());
                let mesh = crate::mesher::mesh_parts(hood, job.field, job.key.0, job.key.1);
                if done.send((job.key, job.seq, mesh)).is_err() {
                    return;
                }
            });
        }
        MeshPool { tx, rx, latest: HashMap::new(), next: 0, out: 0, workers }
    }

    /// How many threads a machine like this one gets: one fewer than it has
    /// cores, two at most with less memory (the chunk generator has its own).
    pub fn workers_for(low_memory: bool) -> usize {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
        cores.saturating_sub(1).clamp(1, if low_memory { 2 } else { 4 })
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Room for more jobs? (Few are kept waiting, so the nearest chunks,
    /// sent first, come back first.)
    pub fn has_room(&self) -> bool {
        self.out < self.workers * 3
    }

    /// Start meshing a chunk, from a copy of it and its neighbours.
    pub fn send(&mut self, world: &World, key: Key) {
        let hood = Box::new(crate::mesher::hood_of(world, key.0, key.1).map(|c| c.cloned()));
        let field = crate::tint::Field::of(world, key.0, key.1);
        self.next += 1;
        self.latest.insert(key, self.next);
        if self.tx.send(Job { key, seq: self.next, hood, field }).is_ok() {
            self.out += 1;
        }
    }

    /// Forget any job out for this chunk (meshed on the spot, or unloaded).
    pub fn cancel(&mut self, key: Key) {
        self.latest.remove(&key);
    }

    /// Forget every job out (another dimension: none of them are wanted).
    pub fn cancel_all(&mut self) {
        self.latest.clear();
    }

    /// Is a mesh being made for this chunk?
    #[cfg(test)]
    pub fn pending(&self, key: Key) -> bool {
        self.latest.contains_key(&key)
    }

    /// Meshes that are done and still wanted.
    pub fn finished(&mut self) -> Vec<(Key, ChunkMesh)> {
        let mut done = Vec::new();
        while let Ok((key, seq, mesh)) = self.rx.try_recv() {
            self.out = self.out.saturating_sub(1);
            if self.latest.get(&key) == Some(&seq) {
                self.latest.remove(&key);
                done.push((key, mesh));
            }
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_meshes_match_meshes_made_on_the_spot_and_stale_ones_are_dropped() {
        let mut w = World::new(5);
        for cz in -1..=1 {
            for cx in -1..=1 {
                w.load_now(cx, cz);
            }
        }
        let here = crate::mesher::mesh_chunk(&w, 0, 0);
        let mut pool = MeshPool::new(2);
        pool.send(&w, (0, 0));
        // Edited while out: the first mesh comes back stale.
        let y = w.surface_y(3, 3) + 1;
        w.set(3, y, 3, crate::block::GLOWROCK);
        pool.send(&w, (0, 0));
        let edited = crate::mesher::mesh_chunk(&w, 0, 0);
        let mut got = Vec::new();
        let start = std::time::Instant::now();
        while got.is_empty() && start.elapsed().as_secs() < 20 {
            got = pool.finished();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Wait out the stale one too.
        while pool.out > 0 && start.elapsed().as_secs() < 20 {
            got.extend(pool.finished());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(got.len(), 1, "only the newest mesh is kept");
        let (key, mesh) = &got[0];
        assert_eq!(*key, (0, 0));
        assert_eq!(mesh.opaque.verts.len(), edited.opaque.verts.len());
        assert_eq!(mesh.lights.len(), edited.lights.len());
        assert!(mesh.lights.len() > here.lights.len(), "the glowrock's light is in it");
        assert!(!pool.pending((0, 0)));
    }
}
