use std::collections::VecDeque;

pub trait Worker: Sized {
    type Event;

    fn handle_event<P: FnMut(Self::Event)>(&mut self, push: P, event: Self::Event);
}

pub struct WorkQueue<W: Worker> {
    worker: W,
    queue: VecDeque<W::Event>,
}

impl<W: Worker> WorkQueue<W> {
    pub fn new(worker: W) -> WorkQueue<W> {
        WorkQueue {
            worker,
            queue: VecDeque::new(),
        }
    }

    pub fn push_event(&mut self, event: W::Event) {
        self.queue.push_back(event);
    }

    pub fn run(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            let queue = &mut self.queue;
            let push = |event| queue.push_back(event);
            self.worker.handle_event(push, event);
        }
    }
}
