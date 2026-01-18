//! Doubly-linked list.
//!
//! The goal here is to create a fast and efficient linked list.
//! Lists use an array of nodes as memory pool, the array must be static.
//!
//! In contrast to [`std::collections::LinkedList`](https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html)
//! you will only ever get a reference to a node and never a copy/move.
//!
//! # Atomicity
//! In an attempt to reduce interrupt latency and with multicore systems in
//! mind, the linked list uses atomic operations. However, these are not safe
//! yet. Use a critical section when accessing the linked list.

#![allow(unused)]

use crate::alloc::allocator::{AllocError, Allocator};
use crate::mem::boxed::Box;
use core::borrow::BorrowMut;
use core::cell::RefCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use core::{mem, ptr};

/******************************************************************************/

type Link<T> = AtomicPtr<Node<T>>;

/// An element/node of a list.
// Copy needed for initialization
#[derive(Debug)]
pub struct Node<T> {
    inner: T,
    prev: Link<T>,
    next: Link<T>,
}

impl<T> Node<T> {
    /// Create a node from an element
    pub const fn new(element: T) -> Self {
        Node {
            inner: element,
            prev: AtomicPtr::new(ptr::null_mut()),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Node<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/******************************************************************************/

/// A doubly-linked list owning its nodes.
///
/// Based on [std::collections::LinkedList](https://doc.rust-lang.org/alloc/collections/linked_list/struct.LinkedList.html)
/// and <https://rust-unofficial.github.io/too-many-lists>.
///
/// # Examples
///
/// Create a new list:
/// ```ignore
/// let mut list_a = LinkedList::new();
/// let mut list_b = LinkedList::new();
/// ```
///
/// Add element to the end of a list with an allocator:
/// ```ignore
/// static ALLOCATOR: Bump = unsafe { Bump::new(NonNull::new_unchecked(0x2001E000 as *mut u8), 5_000) };
/// list_a.emplace_back(MyStruct { id: 42 }, &ALLOCATOR);
/// list_a.emplace_back(MyStruct { id: 54 }, &ALLOCATOR);
///```
/// Nodes in the same list can be allocated in different memory sections.
///
/// Move an element from one to another list:
/// ```ignore
/// let node = list_a.pop_front();
/// list_a.push_back(node);
///```
#[derive(Debug)]
pub struct LinkedList<T> {
    head: Link<T>,
    tail: Link<T>,
    len: AtomicUsize,
}

impl<T> LinkedList<T> {
    /// Create an empty list
    pub const fn new() -> Self {
        LinkedList {
            head: AtomicPtr::new(ptr::null_mut()),
            tail: AtomicPtr::new(ptr::null_mut()),
            len: AtomicUsize::new(0),
        }
    }

    /// Allocate a new element and move it to the end of the list
    ///
    /// **Note:** This fails when we're out of memory
    pub fn emplace_back(
        &self,
        element: T,
        alloc: &'static dyn Allocator,
    ) -> Result<(), AllocError> {
        let node = Box::try_new_in(Node::new(element), alloc);
        // Note(unsafe): map() is only called if the pointer is non-null.
        node.map(|mut n| unsafe {
            self.push_back(n);
        })
    }

    /// Insert a node at the end on the list
    pub fn push_back(&self, mut node: Box<Node<T>>) {
        let mut node_raw = Box::leak(node);
        let mut tail = self.tail.load(Ordering::Acquire);

        // Note(unsafe): Pointer requirements are met.
        unsafe {
            (*node_raw.as_ref()).prev.store(tail, Ordering::Relaxed);
            (*node_raw.as_ref())
                .next
                .store(ptr::null_mut(), Ordering::Relaxed);

            match tail.as_mut() {
                None => self.head.store(node_raw.as_ptr(), Ordering::Relaxed),
                Some(tail) => (*tail).next.store(node_raw.as_ptr(), Ordering::Relaxed),
            };
        }

        self.tail.store(node_raw.as_ptr(), Ordering::Release);
        self.len.fetch_add(1, Ordering::Relaxed);
    }

    /// Remove and return the first node from the list if there is any
    pub fn pop_front(&self) -> Option<Box<Node<T>>> {
        // Note(unsafe): Pointer requirements are met.
        unsafe {
            self.head.load(Ordering::Relaxed).as_mut().map(|node| {
                let next = (*node).next.load(Ordering::Relaxed);
                self.head.store(next, Ordering::Relaxed);

                if let Some(head) = next.as_mut() {
                    (*head).prev.store(ptr::null_mut(), Ordering::Relaxed);
                }

                if self.tail.load(Ordering::Acquire) == node {
                    self.tail.store(next, Ordering::Release);
                }

                (*node).next.store(ptr::null_mut(), Ordering::Relaxed);
                (*node).prev.store(ptr::null_mut(), Ordering::Relaxed);
                self.len.fetch_sub(1, Ordering::Relaxed);
                Box::from_raw(NonNull::new_unchecked(node))
            })
        }
    }

    /// Insert a node exactly before a given node
    ///
    /// **Note:** prefer [`Self::insert_when()`] if possible
    pub fn insert(&self, node: NonNull<Node<T>>, mut new_node: Box<Node<T>>) {
        let node_ptr = node;
        let new_node_ptr = Box::leak(new_node);

        // Note(unsafe): Pointer requirements are met.
        unsafe {
            match (*node_ptr.as_ref()).prev.load(Ordering::Acquire).as_mut() {
                None => {
                    self.head.store(new_node_ptr.as_ptr(), Ordering::Relaxed);
                    (*new_node_ptr.as_ref())
                        .prev
                        .store(ptr::null_mut(), Ordering::Relaxed);
                }
                Some(prev) => {
                    (*prev).next.store(new_node_ptr.as_ptr(), Ordering::Relaxed);
                    (*new_node_ptr.as_ref()).prev.store(prev, Ordering::Relaxed);
                }
            }

            (*node_ptr.as_ref())
                .prev
                .store(new_node_ptr.as_ptr(), Ordering::Release);
            (*new_node_ptr.as_ref())
                .next
                .store(node_ptr.as_ptr(), Ordering::Relaxed);
        }

        self.len.fetch_add(1, Ordering::Relaxed);
    }

    /// Insert a node before the first succeeding match given a comparison criteria.
    ///
    /// # Example
    /// Insert task `pausing` before the element where the next wake-up time
    /// `next_wut()` is larger than the one of `pausing`.
    /// ```ignore
    /// /* create and populate list */
    /// let pausing: Task = /* omitted */;
    /// tasks_sleeping.insert_when(
    ///     pausing,
    ///     |pausing, task| {
    ///         pausing.next_wut() > task.next_wut()
    ///     });
    /// ```
    pub fn insert_when(&self, mut node: Box<Node<T>>, criteria: impl Fn(&T, &T) -> bool) {
        // Note(unsafe): Pointer requirements are met.
        let mut current = unsafe { self.head.load(Ordering::Relaxed).as_mut() };
        if let Some(mut current) = current {
            loop {
                // Note(unsafe): current is checked to be non-null above.
                unsafe {
                    if criteria(&(*node).inner, &current.inner) {
                        self.insert(NonNull::new_unchecked(current), node);
                        return;
                    }

                    current = match current.next.load(Ordering::Relaxed).as_mut() {
                        None => break,
                        Some(next) => next,
                    };
                }
            }
        }
        self.push_back(node);
    }

    /// Get a reference to the first value of the list if there is a node
    pub fn front(&self) -> Option<&T> {
        // Note(unsafe): Pointer requirements are met.
        unsafe {
            self.head
                .load(Ordering::Relaxed)
                .as_mut()
                .map(|head| &(*head).inner)
        }
    }

    /// Get a reference to last value of the list if there is a node
    pub fn back(&self) -> Option<&T> {
        // Note(unsafe): Pointer requirements are met.
        unsafe {
            self.tail
                .load(Ordering::Relaxed)
                .as_mut()
                .map(|tail| &(*tail).inner)
        }
    }

    /// Get the current length of the list
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }

    /// Remove a node from any point in the list.
    ///
    /// # Safety
    /// A node is only allowed to be unliked once.
    unsafe fn unlink(&self, node: Box<Node<T>>) -> Box<Node<T>> {
        self.unlink_raw(Box::leak(node))
    }

    /// Remove a node from any point in the list.
    ///
    /// # Safety
    /// - A node is only allowed to be unliked once.
    /// - Must unlinked in the correct list.
    unsafe fn unlink_raw(&self, mut node: NonNull<Node<T>>) -> Box<Node<T>> {
        let prev = (*node.as_mut()).prev.load(Ordering::Relaxed);
        let next = (*node.as_mut()).next.load(Ordering::Relaxed);

        match prev.as_mut() {
            None => self.head.store(next, Ordering::Relaxed),
            Some(prev) => prev.next.store(next, Ordering::Relaxed),
        };

        match next.as_mut() {
            None => self.tail.store(prev, Ordering::Relaxed),
            Some(next) => next.prev.store(prev, Ordering::Relaxed),
        };

        (*node.as_mut())
            .prev
            .store(ptr::null_mut(), Ordering::Relaxed);
        (*node.as_mut())
            .next
            .store(ptr::null_mut(), Ordering::Relaxed);
        self.len.fetch_sub(1, Ordering::Relaxed);

        Box::from_raw(node)
    }

    /// Provides a forward iterator.
    pub fn iter(&self) -> Iter<'_, T> {
        // Note(unsafe): Pointer requirements are met.
        let next = unsafe { self.head.load(Ordering::Relaxed).as_ref() };
        Iter { next }
    }

    /// Provides a forward iterator with mutable references.
    pub fn iter_mut(&self) -> IterMut<'_, T> {
        // Note(unsafe): Pointer requirements are met.
        let next = unsafe { self.head.load(Ordering::Relaxed).as_mut() };
        IterMut { next }
    }

    /// Provides a cursor with editing operation at the front element.
    pub fn cursor_front_mut(&self) -> Cursor<'_, T> {
        Cursor {
            node: self.head.load(Ordering::Relaxed),
            list: self,
        }
    }
}

/******************************************************************************/

/// An iterator over the elements of a [`LinkedList`].
///
/// This `struct` is created by [`LinkedList::iter()`].
pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.map(|node| unsafe {
            // Note(unsafe): Pointer requirements are met.
            self.next = unsafe { (*node).next.load(Ordering::Relaxed).as_ref() };
            &(*node).inner
        })
    }
}

/// An mutable iterator over the elements of a [`LinkedList`].
///
/// This `struct` is created by [`LinkedList::iter_mut()`].
pub struct IterMut<'a, T> {
    next: Option<&'a mut Node<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.take().map(|node| unsafe {
            // Note(unsafe): Pointer requirements are met.
            self.next = unsafe { (*node).next.load(Ordering::Relaxed).as_mut() };
            &mut (*node).inner
        })
    }
}

/******************************************************************************/

/// A cursor over a [`LinkedList`] with editing operations.
///
/// In contrast to an iterator a cursor can move from front to back and take an
/// element out of the list.
#[derive(Debug)]
pub struct Cursor<'a, T> {
    node: *mut Node<T>,
    list: &'a LinkedList<T>,
}

impl<'a, T> Cursor<'a, T> {
    /// Get reference to value of node if there is any
    pub fn inner(&self) -> Option<&T> {
        // Note(unsafe): Pointer requirements are met.
        unsafe { self.node.as_ref().map(|node| &(*node).inner) }
    }

    /// Get mutable reference to value of node if there is any
    pub fn inner_mut(&self) -> Option<&mut T> {
        // Note(unsafe): Pointer requirements are met.
        unsafe { self.node.as_mut().map(|node| &mut (*node).inner) }
    }

    /// Get raw pointer of node. Only use if you really have to.
    pub(crate) unsafe fn node(&self) -> *mut Node<T> {
        self.node
    }

    /// Move cursor to the next node
    pub fn move_next(&mut self) {
        // Note(unsafe): Pointer requirements are met.
        unsafe {
            if let Some(node) = self.node.as_mut() {
                self.node = (*node).next.load(Ordering::Relaxed);
            }
        }
    }

    /// Take the current node if there is one. Also moves the cursor before
    /// removing a node.
    pub fn take(&mut self) -> Option<Box<Node<T>>> {
        let node = self.node;
        self.move_next();
        // Note(unsafe): Node is checked be non-null.
        unsafe {
            node.as_mut()
                .map(move |node| self.list.unlink_raw(NonNull::new_unchecked(node)))
        }
    }
}