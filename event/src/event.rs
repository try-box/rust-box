trait ListenFn<P, N, R>: 'static + Send + Sync + Fn(P, Option<&N>) -> R {}

impl<T, P, N, R> ListenFn<P, N, R> for T
    where
        T: 'static + ?Sized + Send + Sync + Fn(P, Option<&N>) -> R,
{}

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
        Self {
            l,
        }
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

impl<P, R> Listen<P, R>
{
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







