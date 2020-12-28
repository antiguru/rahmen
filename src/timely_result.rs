use timely::dataflow::operators::{Filter, Map};
use timely::dataflow::{Scope, Stream};
use timely::Data;

pub trait ResultStream<S: Scope, T: Data, E: Data> {
    fn ok(&self) -> Stream<S, T>;
    fn err(&self) -> Stream<S, E>;
    fn map_ok<T2: Data, L: FnMut(T) -> T2 + 'static>(&self, logic: L) -> Stream<S, Result<T2, E>>;
    fn map_err<E2: Data, L: FnMut(E) -> E2 + 'static>(&self, logic: L) -> Stream<S, Result<T, E2>>;
    fn and_then<T2: Data, L: FnMut(T) -> Result<T2, E> + 'static>(
        &self,
        logic: L,
    ) -> Stream<S, Result<T2, E>>;
    fn unwrap_or_else<L: FnMut(E) -> T + 'static>(&self, logic: L) -> Stream<S, T>;
}

impl<S: Scope, T: Data, E: Data> ResultStream<S, T, E> for Stream<S, Result<T, E>> {
    fn ok(&self) -> Stream<S, T> {
        self.filter(|r| r.is_ok()).map(|r| r.ok().unwrap())
    }

    fn err(&self) -> Stream<S, E> {
        self.filter(|r| r.is_err()).map(|r| r.err().unwrap())
    }

    fn map_ok<T2: Data, L: FnMut(T) -> T2 + 'static>(
        &self,
        mut logic: L,
    ) -> Stream<S, Result<T2, E>> {
        self.map(move |r| r.map(|x| logic(x)))
    }

    fn map_err<E2: Data, L: FnMut(E) -> E2 + 'static>(
        &self,
        mut logic: L,
    ) -> Stream<S, Result<T, E2>> {
        self.map(move |r| r.map_err(|x| logic(x)))
    }

    fn and_then<T2: Data, L: FnMut(T) -> Result<T2, E> + 'static>(
        &self,
        mut logic: L,
    ) -> Stream<S, Result<T2, E>> {
        self.map(move |r| r.and_then(|x| logic(x)))
    }

    fn unwrap_or_else<L: FnMut(E) -> T + 'static>(&self, mut logic: L) -> Stream<S, T> {
        self.map(move |r| r.unwrap_or_else(|err| logic(err)))
    }
}
