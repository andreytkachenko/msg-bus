use std::sync::atomic::{AtomicPtr, Ordering};
use std::{ptr, mem};

#[derive(Debug)]
struct Node<T> {
    value: T,
    next: List<T>
}

type NodePtr<T> = Option<Box<Node<T>>>;

#[derive(Debug)]
pub struct List<T>(AtomicPtr<Node<T>>);

impl <T> Default for List<T> {
    fn default() -> Self {
        Self::new_internal(None)
    }
}

impl <T> List<T> {
    #[inline]
    fn into_raw(ptr: NodePtr<T>) -> *mut Node<T> {
        match ptr {
            Some(b) => Box::into_raw(b),
            None => ptr::null_mut()
        }
    }
    
    #[inline]
    unsafe fn from_raw(ptr: *mut Node<T>) -> NodePtr<T> {
        if ptr == ptr::null_mut() {
            None
        } else {
            Some(Box::from_raw(ptr))
        }
    }

    #[inline]
    fn new_internal(ptr: NodePtr<T>) -> Self {
        List(AtomicPtr::new(Self::into_raw(ptr)))
    }

    #[inline]
    pub fn new() -> Self {
        Self::new_internal(None)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.load(Ordering::Relaxed).is_null()
    }
    
    #[inline]
    pub fn append(&self, value: T) {
        unsafe {
            self.append_ptr(List::into_raw(Some(Box::new(Node {
                value,
                next: List::new()
            }))))
        }
    }
    
    pub fn append_list(&self, other: List<T>) {
        let p = other.0.load(Ordering::Relaxed);
        mem::forget(other);
        unsafe { self.append_ptr(p) };
    }

    unsafe fn append_ptr(&self, p: *mut Node<T>) {
        loop {
            match self.0.compare_exchange_weak(ptr::null_mut(), p, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => return,
                Err(head) => if !head.is_null() {
                    return (*head).next.append_ptr(p);
                }
            }
        }
    }

    #[inline]
    pub fn iter(&self) -> ListIterator<T> {
        ListIterator(&self.0)
    }
}


impl<'a, T> IntoIterator for &'a List<T> {
    type Item = &'a T;
    type IntoIter = ListIterator<'a, T>;

    fn into_iter(self) -> ListIterator<'a, T> {
        self.iter()
    }
}

impl<T> Drop for List<T> {
    fn drop(&mut self) {
        unsafe { Self::from_raw(self.0.swap(ptr::null_mut(), Ordering::Relaxed)) };
    }
}


#[derive(Debug)]
pub struct ListIterator<'a, T: 'a>(&'a AtomicPtr<Node<T>>);

impl<'a, T: 'a> Iterator for ListIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let p = self.0.load(Ordering::Relaxed);
        if p.is_null() {
            None
        } else {
            unsafe {
                self.0 = &(*p).next.0;
                Some(&(*p).value)
            }
        }
    }
}
