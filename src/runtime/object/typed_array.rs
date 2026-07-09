use std::{cell::RefCell, rc::Rc};

use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ByteBufferOrigin {
    EngineOwned,
    HostProvided,
}

#[derive(Debug, Clone)]
pub struct ByteBuffer {
    bytes: Rc<RefCell<Vec<u8>>>,
    origin: ByteBufferOrigin,
}

impl ByteBuffer {
    pub fn new(length: usize, origin: ByteBufferOrigin) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![0; length])),
            origin,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>, origin: ByteBufferOrigin) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(bytes)),
            origin,
        }
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.borrow().len()
    }

    pub fn read(&self, index: usize) -> Result<u8> {
        self.bytes
            .borrow()
            .get(index)
            .copied()
            .ok_or_else(|| Error::runtime("typed array byte index is out of bounds"))
    }

    pub fn write(&self, index: usize, value: u8) -> Result<()> {
        let mut bytes = self.bytes.borrow_mut();
        let Some(slot) = bytes.get_mut(index) else {
            return Err(Error::runtime("typed array byte index is out of bounds"));
        };
        *slot = value;
        Ok(())
    }

    pub const fn origin(&self) -> &ByteBufferOrigin {
        &self.origin
    }
}

#[derive(Debug, Clone)]
pub struct Uint8ArrayView {
    buffer: ByteBuffer,
    buffer_object: ObjectId,
    byte_offset: usize,
    length: usize,
}

impl Uint8ArrayView {
    pub const fn new(
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        length: usize,
    ) -> Self {
        Self {
            buffer,
            buffer_object,
            byte_offset,
            length,
        }
    }

    pub const fn length(&self) -> usize {
        self.length
    }

    pub const fn byte_length(&self) -> usize {
        self.length
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn buffer_object(&self) -> ObjectId {
        self.buffer_object
    }

    pub fn read(&self, index: usize) -> Result<Option<u8>> {
        if index >= self.length {
            return Ok(None);
        }
        let absolute = self
            .byte_offset
            .checked_add(index)
            .ok_or_else(|| Error::limit("typed array byte index exceeded supported range"))?;
        self.buffer.read(absolute).map(Some)
    }

    pub fn write(&self, index: usize, value: u8) -> Result<bool> {
        if index >= self.length {
            return Ok(false);
        }
        let absolute = self
            .byte_offset
            .checked_add(index)
            .ok_or_else(|| Error::limit("typed array byte index exceeded supported range"))?;
        self.buffer.write(absolute, value)?;
        Ok(true)
    }
}

impl ObjectHeap {
    pub(crate) fn create_array_buffer(
        &mut self,
        buffer: ByteBuffer,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.byte_buffer = Some(buffer);
        self.push_object(object, max_objects)
    }

    pub(crate) fn create_uint8_array(
        &mut self,
        view: Uint8ArrayView,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.uint8_array = Some(view);
        self.push_object(object, max_objects)
    }

    pub(crate) fn array_buffer(&self, id: ObjectId) -> Result<Option<ByteBuffer>> {
        Ok(self.object(id)?.byte_buffer.clone())
    }

    pub(crate) fn uint8_array(&self, id: ObjectId) -> Result<Option<Uint8ArrayView>> {
        Ok(self.object(id)?.uint8_array.clone())
    }

    pub(crate) fn uint8_array_byte(&self, id: ObjectId, index: usize) -> Result<Option<u8>> {
        let Some(view) = self.object(id)?.uint8_array.as_ref() else {
            return Ok(None);
        };
        view.read(index)
    }

    pub(crate) fn set_uint8_array_byte(
        &self,
        id: ObjectId,
        index: usize,
        value: u8,
    ) -> Result<bool> {
        let Some(view) = self.object(id)?.uint8_array.as_ref() else {
            return Ok(false);
        };
        view.write(index, value)
    }

    pub(crate) fn typed_array_debug_origin(
        &self,
        id: ObjectId,
    ) -> Result<Option<&ByteBufferOrigin>> {
        let Some(view) = self.object(id)?.uint8_array.as_ref() else {
            return Ok(None);
        };
        Ok(Some(view.buffer.origin()))
    }
}

pub fn byte_number(value: &Value) -> Result<u8> {
    let unsigned =
        crate::runtime::numeric::number_to_uint32(value.as_number().unwrap_or(0.0), "Uint8Array")?;
    let byte = unsigned % 256;
    u8::try_from(byte)
        .map_err(|_| Error::runtime("Uint8Array byte conversion exceeded supported range"))
}
