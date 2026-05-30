trait ListenFn<P, N, R>: 'static + Send + Sync + Fn(P, Option<&N>) -> R {}

impl<T, P, N, R> ListenFn<P, N, R> for T where
    T: 'static + ?Sized + Send + Sync + Fn(P, Option<&N>) -> R
{
}

pub struct Event<P, R> {
    l: Listen<P, R>,
}

impl<P, R> Event<P, R> {
    #[inline]
    pub fn listen<F>(f: F) -> Listen<P, R>
    where
        F: Fn(P, Option<&Next<P, R>>) -> R + Send + Sync + 'static,
    {
        Listen::new(f)
    }

    #[inline]
    pub fn fire(&self, args: P) -> R {
        let n = self.l.next.as_ref().map(|n| n.as_ref());
        (self.l.f)(args, n)
    }
}

pub struct Next<P, R> {
    l: Listen<P, R>,
}

impl<P, R> Next<P, R> {
    #[inline]
    fn new(l: Listen<P, R>) -> Self {
        Self { l }
    }

    #[inline]
    pub fn forward(&self, args: P) -> R {
        let n = self.l.next.as_ref().map(|n| n.as_ref());
        (self.l.f)(args, n)
    }

    #[inline]
    fn link(&mut self, next: Self) {
        if let Some(n) = self.l.next.as_mut() {
            n.link(next);
        } else {
            self.l.next = Some(Box::new(next));
        }
    }
}

pub struct Listen<P, R> {
    f: Box<dyn ListenFn<P, Next<P, R>, R>>,
    next: Option<Box<Next<P, R>>>,
}

impl<P, R> Listen<P, R> {
    #[inline]
    fn new<F>(f: F) -> Self
    where
        F: Fn(P, Option<&Next<P, R>>) -> R + Send + Sync + 'static,
    {
        Self {
            f: Box::new(f),
            next: None,
        }
    }

    #[inline]
    pub fn listen<F>(mut self, f: F) -> Self
    where
        F: Fn(P, Option<&Next<P, R>>) -> R + Send + Sync + 'static,
    {
        let next = Next::new(Self::new(f));
        if let Some(n) = self.next.as_mut() {
            n.link(next);
        } else {
            self.next = Some(Box::new(next));
        }
        self
    }

    #[inline]
    pub fn finish(self) -> Event<P, R> {
        Event { l: self }
    }
}

#[test]
fn test_event() {
    let simple1 = Event::<i32, i32>::listen(|args: i32, next| {
        if args == 100 {
            return args;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            1 * args
        }
    })
    .listen(|args: i32, next| {
        if args == 200 {
            return args;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            2 * args
        }
    })
    .listen(|args: i32, next| {
        if args == 300 {
            return args;
        }
        if let Some(next) = next {
            next.forward(args)
        } else {
            3 * args
        }
    })
    .finish();

    assert_eq!(simple1.fire(10), 30);
    assert_eq!(simple1.fire(100), 100);
    assert_eq!(simple1.fire(300), 300);
}

#[test]
fn test_event_single_listener() {
    let event = Event::<i32, i32>::listen(|args, _next| args * 2).finish();

    assert_eq!(event.fire(5), 10);
    assert_eq!(event.fire(21), 42);
}

#[test]
fn test_event_no_listener() {
    // Single listener with no chained next (next = None)
    let event = Event::<i32, i32>::listen(|args, next| {
        assert!(next.is_none());
        args * 3
    })
    .finish();

    assert_eq!(event.fire(7), 21);
}

#[test]
fn test_event_chained() {
    // Extended chain: 4 listeners, value passes through
    let event = Event::<i32, i32>::listen(|args: i32, next| {
        if let Some(next) = next {
            next.forward(args + 1)
        } else {
            args
        }
    })
    .listen(|args: i32, next| {
        if let Some(next) = next {
            next.forward(args + 2)
        } else {
            args
        }
    })
    .listen(|args: i32, next| {
        if let Some(next) = next {
            next.forward(args + 3)
        } else {
            args
        }
    })
    .listen(|args: i32, _next| args + 4)
    .finish();

    // 10 + 1 + 2 + 3 + 4 = 20
    assert_eq!(event.fire(10), 20);
    // 0 + 1 + 2 + 3 + 4 = 10
    assert_eq!(event.fire(0), 10);
}

#[test]
fn test_event_chain_broken() {
    // Middle listener does not forward, breaking the chain
    let event = Event::<i32, String>::listen(|args: i32, next| {
        if let Some(next) = next {
            next.forward(args)
        } else {
            "end".to_string()
        }
    })
    .listen(|args: i32, _next| {
        // Does not forward, chain breaks here
        format!("broken at {}", args)
    })
    .listen(|_args: i32, _next| unreachable!("this listener should never be called"))
    .finish();

    assert_eq!(event.fire(42), "broken at 42");
}

#[test]
fn test_event_with_data() {
    // Parameterized event with String data
    let event = Event::<String, usize>::listen(|args: String, next| {
        if let Some(next) = next {
            next.forward(args)
        } else {
            args.len()
        }
    })
    .listen(|args: String, _next| args.len() * 2)
    .finish();

    assert_eq!(event.fire("hello".to_string()), 10);
    assert_eq!(event.fire("rust".to_string()), 8);
}

#[test]
fn test_event_multiple_fire() {
    // Fire same event multiple times, should produce same results
    let event = Event::<i32, i32>::listen(|args: i32, next| {
        if let Some(next) = next {
            next.forward(args * 2)
        } else {
            args
        }
    })
    .listen(|args: i32, _next| args + 1)
    .finish();

    // (5 * 2) + 1 = 11
    assert_eq!(event.fire(5), 11);
    // (5 * 2) + 1 = 11 (same args, same result)
    assert_eq!(event.fire(5), 11);
    // (3 * 2) + 1 = 7
    assert_eq!(event.fire(3), 7);
    assert_eq!(event.fire(10), 21);
}
