#![feature(macro_rules)]

use std::collections::HashMap;

const PAGE_SIZE: uint = 0x1000;
const MASK: [u8, ..8] = [ 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80 ];

macro_rules! align_x(($inp:expr, $alp:expr) => (($inp) & (0 - $alp)))
macro_rules! align_next_x(($inp:expr, $alp:expr) => (($inp + ($alp - 1)) & (0 - $alp)))
macro_rules! align_8(($inp:expr) => (align_x!($inp, 8)))
macro_rules! align_next_8(($inp:expr) => (align_next_x!($inp, 8)))

struct ShadowPage {
    buf: [u8, ..PAGE_SIZE],
    map: [u8, ..PAGE_SIZE / 8],
}

impl ShadowPage {
    fn has_patch(&self, rel_offs: uint) -> bool {
        let bit_idx = rel_offs / 8;
        if self.map[bit_idx] == 0 {
            return false;
        }

        self.map[bit_idx] & MASK[rel_offs % 8] != 0
    }

    fn has_patch_in_range(&self, (beg, end): (uint, uint)) -> bool {
        let mut i = beg;

        while i <= end {
            let i_aligned = align_8!(i);
            if self.map[i_aligned / 8] > 0 {
                let bit_idx = i - i_aligned;
                if self.map[i_aligned / 8] & MASK[bit_idx] != 0 {
                    return true;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
                i = align_next_8!(i);
            }
        }

        false
    }
}

struct ShadowManager {
    pages: HashMap<u64, ShadowPage>,
}

impl ShadowManager {
    fn new() -> ShadowManager {
        ShadowManager {
            pages: HashMap::new(),
        }
    }

    fn add_byte(&mut self, offset: u64, byte: u8) {
        let page_offs = self.get_page_offset(offset);
        let rel_offs = (offset - page_offs) as uint;
        let (_, bit_idx) = self.get_bit_index(rel_offs);

        if ! self.pages.contains_key(&page_offs) {
            let nsp = ShadowPage { buf: [0, ..PAGE_SIZE], map: [0, ..PAGE_SIZE / 8] };
            self.pages.insert(page_offs, nsp);
        }

        let shadow_page = match self.pages.get_mut(&page_offs) {
            Some(x) => x,
            None => return,
        };

        shadow_page.buf[rel_offs] = byte;
        shadow_page.map[rel_offs / 8] |= 1 << bit_idx;
    }

    #[allow(dead_code)]
    fn debug_dump(&self) {
        println!("--> There are {} shadow pages in repo.", self.pages.len());
        for (offset, sm) in self.pages.iter() {
            println!("offset={:x}, shadow {:p}", *offset, sm);
            self.debug_dump_page(sm);
        }
    }

    fn debug_dump_page(&self, page: &ShadowPage) {
        for i in range(0, PAGE_SIZE) {
            print!("{:02x} ", page.buf[i]);
        }

        println!("");
    }

    fn has_patch(&self, abs_offset: u64) -> bool {
        let page_offs = self.get_page_offset(abs_offset);
        let rel_offs = (abs_offset - page_offs) as uint;

        match self.pages.get(&page_offs) {
            Some(sm) => sm.has_patch(rel_offs),
            None => false,
        }
    }

    fn has_patch_in_range(&self, (beg, end): (u64, u64)) -> bool {
        let mut cur_page = self.get_page_offset(beg);
        let last_page = self.get_page_offset(end);

        let mut on_first_page = true;

        loop {
            let on_last_page = cur_page == last_page;

            let sm = match self.pages.get(&cur_page) {
                None => return false,
                Some(x) => x,
            };

            if on_first_page && on_last_page {
                let beg_rel = (beg - cur_page) as uint;
                let end_rel = (end - cur_page) as uint;

                if beg_rel == end_rel {
                    return sm.has_patch(beg_rel);
                } else {
                    return sm.has_patch_in_range((beg_rel, end_rel));
                }
            }

            if on_first_page {
                on_first_page = false;

                let rel_from = beg - cur_page;
                if sm.has_patch_in_range((rel_from as uint, PAGE_SIZE)) {
                    return true;
                }
            }

            if on_last_page {
                let rel_to = end - cur_page;
                return sm.has_patch_in_range((0, rel_to as uint));
            }

            cur_page += PAGE_SIZE as u64;
        }
    }

    fn debug_dump_offsets(&self) {
        for (offset, _) in self.pages.iter() {
            for i in range(0, PAGE_SIZE) {
                let abs_offs = *offset + i as u64;

                if self.has_patch(abs_offs) {
                    println!("got patch @ {:x}", abs_offs);
                }
            }
        }
    }

    fn get_page_offset(&self, offset: u64) -> u64 {
        offset & (-1 as u64 - PAGE_SIZE as u64 + 1)
    }

    fn get_bit_index(&self, rel_offs: uint) -> (uint, uint) {
        (rel_offs / 8, (rel_offs % 8) as uint)
    }
}

fn main() {
    let mut sm = ShadowManager::new();
    sm.add_byte(0x2123, 0xa1);
    sm.add_byte(0x1123, 0xa1);
    sm.add_byte(0, 0xde);
    sm.add_byte(100, 0xad);
    sm.add_byte(7, 7);
    sm.add_byte(8, 8);

    sm.debug_dump_offsets();
}

#[test]
fn test_range_1() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1000, 1);
    assert!(sm.has_patch_in_range((999, 1001)) == true);
}

#[test]
fn test_range_2() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1000, 1);
    assert!(sm.has_patch_in_range((1000, 1000)) == true);
}

#[test]
fn test_range_3() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1000, 1);
    assert!(sm.has_patch_in_range((1001, 1001)) == false);
}

#[test]
fn test_range_4() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1000, 1);
    assert!(sm.has_patch_in_range((500, 700)) == false);
}

#[test]
fn test_range_5() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1, 1);
    assert!(sm.has_patch_in_range((0, 1)) == true);
}

#[test]
fn test_0() {
    let mut sm = ShadowManager::new();
    sm.add_byte(0, 1);
    assert!(sm.has_patch(0) == true);
}

#[test]
fn test_range_0() {
    let mut sm = ShadowManager::new();
    sm.add_byte(0, 1);
    assert!(sm.has_patch_in_range((0, 0)) == true);
}

#[test]
fn test_range_0a() {
    let mut sm = ShadowManager::new();
    sm.add_byte(0, 1);
    assert!(sm.has_patch_in_range((0, 1)) == true);
}

#[test]
fn test_range_0b() {
    let mut sm = ShadowManager::new();
    sm.add_byte(0, 1);
    assert!(sm.has_patch_in_range((0, 5)) == true);
}

#[test]
fn test_1() {
    let mut sm = ShadowManager::new();
    sm.add_byte(1000, 1);
    assert!(sm.has_patch((1000)) == true);
}

#[test] fn align_0() { assert!(align_8!(0u) == 0u); }
#[test] fn align_1() { assert!(align_8!(1u) == 0u); }
#[test] fn align_2() { assert!(align_8!(2u) == 0u); }
#[test] fn align_3() { assert!(align_8!(3u) == 0u); }
#[test] fn align_4() { assert!(align_8!(4u) == 0u); }
#[test] fn align_5() { assert!(align_8!(5u) == 0u); }
#[test] fn align_6() { assert!(align_8!(6u) == 0u); }
#[test] fn align_7() { assert!(align_8!(7u) == 0u); }
#[test] fn align_8() { assert!(align_8!(8u) == 8u); }
#[test] fn align_9() { assert!(align_8!(9u) == 8u); }
