// This is a generated file! Please edit source .ksy file and use kaitai-struct-compiler to rebuild

#![allow(unused_imports)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(irrefutable_let_patterns)]
#![allow(unused_comparisons)]

extern crate kaitai;
use kaitai::*;
use std::convert::{TryFrom, TryInto};
use std::cell::{Ref, Cell, RefCell};
use std::rc::{Rc, Weak};

#[derive(Default, Debug, Clone)]
pub struct Demo {
    pub _root: SharedType<Demo>,
    pub _parent: SharedType<Demo>,
    pub _self: SharedType<Self>,
    magic: RefCell<Vec<u8>>,
    version: RefCell<u16>,
    kind: RefCell<Demo_Color>,
    num_items: RefCell<u32>,
    items: RefCell<Vec<OptRc<Demo_Item>>>,
    trailer: RefCell<u8>,
    _io: RefCell<BytesReader>,
    f_computed: Cell<bool>,
    computed: RefCell<i32>,
}
impl KStruct for Demo {
    type Root = Demo;
    type Parent = Demo;

    fn read<S: KStream>(
        self_rc: &OptRc<Self>,
        _io: &S,
        _root: SharedType<Self::Root>,
        _parent: SharedType<Self::Parent>,
    ) -> KResult<()> {
        *self_rc._io.borrow_mut() = _io.clone();
        self_rc._root.set(_root.get());
        self_rc._parent.set(_parent.get());
        self_rc._self.set(Ok(self_rc.clone()));
        let _rrc = self_rc._root.get_value().borrow().upgrade();
        let _prc = self_rc._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        *self_rc.magic.borrow_mut() = _io.read_bytes(4 as usize)?.into();
        if !(*self_rc.magic() == vec![0x44u8, 0x45u8, 0x4du8, 0x4fu8]) {
            return Err(KError::ValidationFailed(ValidationFailedError { kind: ValidationKind::NotEqual, src_path: "/seq/0".to_string() }));
        }
        *self_rc.version.borrow_mut() = _io.read_u2be()?.into();
        *self_rc.kind.borrow_mut() = (_io.read_u1()? as i64).try_into()?;
        *self_rc.num_items.borrow_mut() = _io.read_u4be()?.into();
        *self_rc.items.borrow_mut() = Vec::new();
        let l_items = *self_rc.num_items();
        for _i in 0..l_items {
            let t = Self::read_into::<_, Demo_Item>(&*_io, Some(self_rc._root.clone()), Some(self_rc._self.clone()))?.into();
            self_rc.items.borrow_mut().push(t);
        }
        if ((*self_rc.version() as u16) > (1 as u16)) {
            *self_rc.trailer.borrow_mut() = _io.read_u1()?.into();
        }
        Ok(())
    }
}
impl Demo {
    pub fn computed(
        &self
    ) -> KResult<Ref<'_, i32>> {
        let _io = self._io.borrow();
        let _rrc = self._root.get_value().borrow().upgrade();
        let _prc = self._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        if self.f_computed.get() {
            return Ok(self.computed.borrow());
        }
        self.f_computed.set(true);
        *self.computed.borrow_mut() = (((*self.version() as u16) + (1 as u16))) as i32;
        Ok(self.computed.borrow())
    }
}
impl Demo {
    pub fn magic(&self) -> Ref<'_, Vec<u8>> {
        self.magic.borrow()
    }
}
impl Demo {
    pub fn version(&self) -> Ref<'_, u16> {
        self.version.borrow()
    }
}
impl Demo {
    pub fn kind(&self) -> Ref<'_, Demo_Color> {
        self.kind.borrow()
    }
}
impl Demo {
    pub fn num_items(&self) -> Ref<'_, u32> {
        self.num_items.borrow()
    }
}
impl Demo {
    pub fn items(&self) -> Ref<'_, Vec<OptRc<Demo_Item>>> {
        self.items.borrow()
    }
}
impl Demo {
    pub fn trailer(&self) -> Ref<'_, u8> {
        self.trailer.borrow()
    }
}
impl Demo {
    pub fn _io(&self) -> Ref<'_, BytesReader> {
        self._io.borrow()
    }
}
#[derive(Debug, PartialEq, Clone)]
pub enum Demo_Color {
    Red,
    Green,
    Blue,
    Unknown(i64),
}

impl TryFrom<i64> for Demo_Color {
    type Error = KError;
    fn try_from(flag: i64) -> KResult<Demo_Color> {
        match flag {
            0 => Ok(Demo_Color::Red),
            1 => Ok(Demo_Color::Green),
            2 => Ok(Demo_Color::Blue),
            _ => Ok(Demo_Color::Unknown(flag)),
        }
    }
}

impl From<&Demo_Color> for i64 {
    fn from(v: &Demo_Color) -> Self {
        match *v {
            Demo_Color::Red => 0,
            Demo_Color::Green => 1,
            Demo_Color::Blue => 2,
            Demo_Color::Unknown(v) => v
        }
    }
}

impl Default for Demo_Color {
    fn default() -> Self { Demo_Color::Unknown(0) }
}


#[derive(Default, Debug, Clone)]
pub struct Demo_Item {
    pub _root: SharedType<Demo>,
    pub _parent: SharedType<Demo>,
    pub _self: SharedType<Self>,
    tag: RefCell<u8>,
    body: RefCell<Option<Demo_Item_Body>>,
    _io: RefCell<BytesReader>,
}
#[derive(Debug, Clone)]
pub enum Demo_Item_Body {
    Demo_TextBody(OptRc<Demo_TextBody>),
    Demo_NumBody(OptRc<Demo_NumBody>),
}
impl From<&Demo_Item_Body> for OptRc<Demo_TextBody> {
    fn from(v: &Demo_Item_Body) -> Self {
        if let Demo_Item_Body::Demo_TextBody(x) = v {
            return x.clone();
        }
        panic!("expected Demo_Item_Body::Demo_TextBody, got {:?}", v)
    }
}
impl From<OptRc<Demo_TextBody>> for Demo_Item_Body {
    fn from(v: OptRc<Demo_TextBody>) -> Self {
        Self::Demo_TextBody(v)
    }
}
impl From<&Demo_Item_Body> for OptRc<Demo_NumBody> {
    fn from(v: &Demo_Item_Body) -> Self {
        if let Demo_Item_Body::Demo_NumBody(x) = v {
            return x.clone();
        }
        panic!("expected Demo_Item_Body::Demo_NumBody, got {:?}", v)
    }
}
impl From<OptRc<Demo_NumBody>> for Demo_Item_Body {
    fn from(v: OptRc<Demo_NumBody>) -> Self {
        Self::Demo_NumBody(v)
    }
}
impl KStruct for Demo_Item {
    type Root = Demo;
    type Parent = Demo;

    fn read<S: KStream>(
        self_rc: &OptRc<Self>,
        _io: &S,
        _root: SharedType<Self::Root>,
        _parent: SharedType<Self::Parent>,
    ) -> KResult<()> {
        *self_rc._io.borrow_mut() = _io.clone();
        self_rc._root.set(_root.get());
        self_rc._parent.set(_parent.get());
        self_rc._self.set(Ok(self_rc.clone()));
        let _rrc = self_rc._root.get_value().borrow().upgrade();
        let _prc = self_rc._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        *self_rc.tag.borrow_mut() = _io.read_u1()?.into();
        match *self_rc.tag() {
            0 => {
                let t = Self::read_into::<_, Demo_TextBody>(&*_io, Some(self_rc._root.clone()), Some(self_rc._self.clone()))?.into();
                *self_rc.body.borrow_mut() = Some(t);
            }
            1 => {
                let t = Self::read_into::<_, Demo_NumBody>(&*_io, Some(self_rc._root.clone()), Some(self_rc._self.clone()))?.into();
                *self_rc.body.borrow_mut() = Some(t);
            }
            _ => {}
        }
        Ok(())
    }
}
impl Demo_Item {
}
impl Demo_Item {
    pub fn tag(&self) -> Ref<'_, u8> {
        self.tag.borrow()
    }
}
impl Demo_Item {
    pub fn body(&self) -> Ref<'_, Option<Demo_Item_Body>> {
        self.body.borrow()
    }
}
impl Demo_Item {
    pub fn _io(&self) -> Ref<'_, BytesReader> {
        self._io.borrow()
    }
}

#[derive(Default, Debug, Clone)]
pub struct Demo_NumBody {
    pub _root: SharedType<Demo>,
    pub _parent: SharedType<Demo_Item>,
    pub _self: SharedType<Self>,
    value: RefCell<u32>,
    _io: RefCell<BytesReader>,
}
impl KStruct for Demo_NumBody {
    type Root = Demo;
    type Parent = Demo_Item;

    fn read<S: KStream>(
        self_rc: &OptRc<Self>,
        _io: &S,
        _root: SharedType<Self::Root>,
        _parent: SharedType<Self::Parent>,
    ) -> KResult<()> {
        *self_rc._io.borrow_mut() = _io.clone();
        self_rc._root.set(_root.get());
        self_rc._parent.set(_parent.get());
        self_rc._self.set(Ok(self_rc.clone()));
        let _rrc = self_rc._root.get_value().borrow().upgrade();
        let _prc = self_rc._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        *self_rc.value.borrow_mut() = _io.read_u4be()?.into();
        Ok(())
    }
}
impl Demo_NumBody {
}
impl Demo_NumBody {
    pub fn value(&self) -> Ref<'_, u32> {
        self.value.borrow()
    }
}
impl Demo_NumBody {
    pub fn _io(&self) -> Ref<'_, BytesReader> {
        self._io.borrow()
    }
}

#[derive(Default, Debug, Clone)]
pub struct Demo_TextBody {
    pub _root: SharedType<Demo>,
    pub _parent: SharedType<Demo_Item>,
    pub _self: SharedType<Self>,
    len: RefCell<u8>,
    value: RefCell<String>,
    _io: RefCell<BytesReader>,
}
impl KStruct for Demo_TextBody {
    type Root = Demo;
    type Parent = Demo_Item;

    fn read<S: KStream>(
        self_rc: &OptRc<Self>,
        _io: &S,
        _root: SharedType<Self::Root>,
        _parent: SharedType<Self::Parent>,
    ) -> KResult<()> {
        *self_rc._io.borrow_mut() = _io.clone();
        self_rc._root.set(_root.get());
        self_rc._parent.set(_parent.get());
        self_rc._self.set(Ok(self_rc.clone()));
        let _rrc = self_rc._root.get_value().borrow().upgrade();
        let _prc = self_rc._parent.get_value().borrow().upgrade();
        let _r = _rrc.as_ref().unwrap();
        *self_rc.len.borrow_mut() = _io.read_u1()?.into();
        *self_rc.value.borrow_mut() = bytes_to_str(&_io.read_bytes(*self_rc.len() as usize)?.into(), "ASCII")?;
        Ok(())
    }
}
impl Demo_TextBody {
}
impl Demo_TextBody {
    pub fn len(&self) -> Ref<'_, u8> {
        self.len.borrow()
    }
}
impl Demo_TextBody {
    pub fn value(&self) -> Ref<'_, String> {
        self.value.borrow()
    }
}
impl Demo_TextBody {
    pub fn _io(&self) -> Ref<'_, BytesReader> {
        self._io.borrow()
    }
}
