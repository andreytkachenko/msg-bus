use crate::list::*;
use core::any::TypeId;
use core::mem::{transmute, MaybeUninit};

pub struct HashMap<V> {
    map: [List<(TypeId, V)>; 256],
}

impl<V> HashMap<V> {
    pub fn new() -> Self {
        let mut data: [MaybeUninit<List<(TypeId, V)>>; 256] =
            unsafe { MaybeUninit::uninit().assume_init() };

        for elem in &mut data[..] {
            unsafe {
                std::ptr::write(elem.as_mut_ptr(), List::new());
            }
        }

        Self {
            map: unsafe { std::mem::transmute::<_, [List<(TypeId, V)>; 256]>(data) },
        }
    }

    pub fn insert(&self, key: TypeId, val: V) {
        let i: u64 = unsafe { transmute(key) };
        let k = (i % 255) as usize;

        self.map[k].append((key, val));
    }

    pub fn items(&self, key: TypeId) -> impl Iterator<Item = &V> {
        let i: u64 = unsafe { transmute(key) };
        let k = (i % 255) as u8;

        self.map[k as usize]
            .iter()
            .filter_map(move |(lkey, v)| if *lkey == key { Some(v) } else { None })
    }

    #[inline]
    pub fn get(&self, k: TypeId) -> Option<&V> {
        self.items(k).next()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &(TypeId, V)> {
        HashMapIter {
            map: &self.map,
            list_iter: None,
            index: 0,
        }
    }
}

pub struct HashMapIter<'a, V> {
    map: &'a [List<(TypeId, V)>; 256],
    list_iter: Option<ListIterator<'a, (TypeId, V)>>,
    index: usize,
}

impl<'a, V> Iterator for HashMapIter<'a, V> {
    type Item = &'a (TypeId, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(li) = &mut self.list_iter {
                match li.next() {
                    Some(x) => return Some(x),
                    None => (),
                }
            }

            if self.index > 255 {
                return None;
            }

            self.list_iter = Some(self.map[self.index].iter());
            self.index += 1;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map() {
        let map = HashMap::new();

        map.insert(TypeId::of::<i32>(), 128);
        map.insert(TypeId::of::<i16>(), 12);
        map.insert(TypeId::of::<String>(), 4);
        map.insert(TypeId::of::<&str>(), 11);

        assert_eq!(map.get(TypeId::of::<i32>()), Some(&128));
        assert_eq!(map.get(TypeId::of::<i16>()), Some(&12));
        assert_eq!(map.get(TypeId::of::<String>()), Some(&4));
        assert_eq!(map.get(TypeId::of::<&str>()), Some(&11));
    }
}
