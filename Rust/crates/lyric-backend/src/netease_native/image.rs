//! PE/RTTI and bounded instruction data flow. Addresses and member displacements
//! come from the client being read, never a client version or image fingerprint.
use std::collections::{BTreeMap,BTreeSet};
use iced_x86::{Decoder,DecoderOptions,Instruction,Mnemonic as M,OpKind as O,Register as R};
use super::{bad,bytes,u16_at,u32_at,u64_at,unique,Result};
pub(super) const READ:u32=0x40000000;
pub(super) const WRITE:u32=0x80000000;
pub(super) const EXEC:u32=0x20000000;
const AUDIO:&[u8]=b".?AVAudioPlayer@audio@@\0";
const INFOS:[&[u8];2]=[b".?AVAudioPlayInfo@player@@\0",b".?AVAudioPlayInfo@plugin@@\0"];

#[derive(Clone)]pub(super) struct Section{pub rva:usize,pub virtual_size:usize,raw:usize,raw_size:usize,pub flags:u32}
impl Section{fn contains(&self,rva:usize,n:usize)->bool{rva>=self.rva&&rva.checked_add(n).is_some_and(|end|end<=self.rva+self.virtual_size)}}
pub(super) struct Image{file:Vec<u8>,pub base:u64,pub bits:u32,pub size:usize,pub timestamp:u32,pub sections:Vec<Section>,functions:Vec<(usize,usize)>}
#[derive(Clone,Debug,PartialEq,Eq,serde::Serialize)]pub(super) struct Layout{
 pub audio:usize,pub info:usize,pub secondary:usize,pub secondary_offset:usize,
 pub state:usize,pub id_fields:Vec<usize>,pub position:usize,pub buffered:usize,pub duration:usize,
}
#[derive(Clone,Debug)]struct Vtable{rva:usize,offset:usize,methods:Vec<usize>}
impl Image{
 pub fn parse(file:Vec<u8>)->Result<Self>{
  if bytes(&file,0,2)?!=b"MZ"{return Err(bad())}let pe=u32_at(&file,0x3c)?as usize;
  if bytes(&file,pe,4)?!=b"PE\0\0"{return Err(bad())}
  let(bits,minimum)=match(u16_at(&file,pe+4)?,u16_at(&file,pe+24)?){(0x8664,0x20b)=>(64,240),(0x14c,0x10b)=>(32,224),_=>return Err(bad())};
  let count=u16_at(&file,pe+6)?as usize;let optional=u16_at(&file,pe+20)?as usize;
  if count==0||count>96||optional<minimum{return Err(bad())}
  let size=u32_at(&file,pe+24+56)?as usize;if !(4096..=256*1024*1024).contains(&size){return Err(bad())}
  let base=if bits==64{u64_at(&file,pe+24+24)?}else{u32_at(&file,pe+24+28)?as u64};
  let mut sections=Vec::new();
  for i in 0..count{let at=pe+24+optional+i*40;let s=Section{virtual_size:u32_at(&file,at+8)?as usize,rva:u32_at(&file,at+12)?as usize,raw_size:u32_at(&file,at+16)?as usize,raw:u32_at(&file,at+20)?as usize,flags:u32_at(&file,at+36)?};
   if s.rva.checked_add(s.virtual_size).is_none_or(|end|end>size){return Err(bad())}bytes(&file,s.raw,s.raw_size)?;
   if sections.iter().any(|o:&Section|s.rva<o.rva+o.virtual_size&&o.rva<s.rva+s.virtual_size){return Err(bad())}sections.push(s);
  }
  let mut image=Self{base,bits,timestamp:u32_at(&file,pe+8)?,size,file,sections,functions:Vec::new()};
  if bits==64{
   let table=u32_at(&image.file,pe+24+112+3*8)?as usize;let size=u32_at(&image.file,pe+24+116+3*8)?as usize;
   if size>8*1024*1024||size%12!=0{return Err(bad())}
   if size>0{let data=image.at(table,size)?;let mut functions=Vec::new();
    for at in (0..data.len()).step_by(12){let start=u32_at(data,at)?as usize;let end=u32_at(data,at+4)?as usize;
     if start>=end||!image.executable(start,end-start){return Err(bad())}functions.push((start,end));
    }functions.sort_unstable();image.functions=functions;
   }
  }Ok(image)
 }
 pub fn ptr(&self)->usize{self.bits as usize/8}
 pub fn word(&self,b:&[u8],at:usize)->Result<u64>{if self.bits==64{u64_at(b,at)}else{Ok(u32_at(b,at)?as u64)}}
 pub fn section(&self,rva:usize,n:usize)->Option<&Section>{self.sections.iter().find(|s|s.contains(rva,n))}
 fn executable(&self,rva:usize,n:usize)->bool{self.section(rva,n).is_some_and(|s|s.flags&EXEC!=0)}
 fn writable(&self,rva:usize,n:usize)->bool{self.section(rva,n).is_some_and(|s|s.flags&WRITE!=0&&s.flags&EXEC==0)}
 fn at(&self,rva:usize,n:usize)->Result<&[u8]>{let s=self.section(rva,n).ok_or_else(bad)?;let d=rva-s.rva;if d.checked_add(n).is_none_or(|end|end>s.raw_size){return Err(bad())}bytes(&self.file,s.raw+d,n)}
 fn rva(&self,address:u64)->Result<usize>{let r=usize::try_from(address.checked_sub(self.base).ok_or_else(bad)?).map_err(|_|bad())?;self.section(r,1).map(|_|r).ok_or_else(bad)}
 fn instructions(&self,rva:usize)->Result<Vec<Instruction>>{
  let s=self.section(rva,1).filter(|s|s.flags&EXEC!=0).ok_or_else(bad)?;
  let limit=(s.rva+s.raw_size).checked_sub(rva).ok_or_else(bad)?.min(4096);
  let n=self.functions.binary_search_by_key(&rva,|f|f.0).ok().map(|i|self.functions[i].1-rva).unwrap_or(limit).min(limit);
  let mut decoder=Decoder::with_ip(self.bits,self.at(rva,n)?,self.base+rva as u64,DecoderOptions::NONE);let mut out=Vec::new();
  while decoder.can_decode()&&out.len()<1024{let i=decoder.decode();if i.is_invalid(){break}let end=i.mnemonic()==M::Ret;out.push(i);if end{break}}
  if out.is_empty(){return Err(bad())}Ok(out)
 }
 fn vtables(&self,name:&[u8])->Result<Vec<Vtable>>{
  let ptr=self.ptr();let mut descriptors=Vec::new();
  for s in &self.sections{if s.flags&READ==0{continue}let data=bytes(&self.file,s.raw,s.raw_size)?;
   for(i,w)in data.windows(name.len()).enumerate(){if w==name&&i>=2*ptr{descriptors.push(s.rva+i-2*ptr)}}
  }
  if descriptors.is_empty(){return Ok(Vec::new())}let td=unique(descriptors)?;let mut locators=Vec::new();
  for s in &self.sections{if s.flags&READ==0||s.flags&(WRITE|EXEC)!=0{continue}let data=bytes(&self.file,s.raw,s.raw_size)?;let n=if self.bits==64{24}else{20};
   for at in (0..data.len().saturating_sub(n-1)).step_by(4){let valid=if self.bits==64{
    u32_at(data,at)?==1&&u32_at(data,at+8)?==0&&u32_at(data,at+12)?as usize==td&&u32_at(data,at+20)?as usize==s.rva+at
   }else{u32_at(data,at)?==0&&u32_at(data,at+8)?==0&&u32_at(data,at+12)?as u64==self.base+td as u64};
    if valid{let offset=u32_at(data,at+4)?as usize;if offset<1024{locators.push((s.rva+at,offset))}}
   }
  }
  let mut tables=Vec::new();
  for(col,offset)in locators{for s in &self.sections{if s.flags&READ==0||s.flags&(WRITE|EXEC)!=0{continue}let data=bytes(&self.file,s.raw,s.raw_size)?;
   for at in (0..data.len().saturating_sub(ptr-1)).step_by(ptr){if self.word(data,at)?!=self.base+col as u64{continue}let rva=s.rva+at+ptr;let mut methods=Vec::new();
    for slot in 0..64{let Ok(data)=self.at(rva+slot*ptr,ptr)else{break};let Ok(fn_rva)=self.rva(self.word(data,0)?)else{break};if !self.executable(fn_rva,1){break}methods.push(fn_rva)}
    if !methods.is_empty(){tables.push(Vtable{rva,offset,methods})}
   }
  }}Ok(tables)
 }
 pub fn discover(&self)->Result<Layout>{
  let audio=unique(self.vtables(AUDIO)?.into_iter().filter(|t|t.offset==0).collect()).map_err(|_|"AudioPlayer RTTI is missing or ambiguous".to_string())?;
  let mut cache=BTreeMap::new();let mut states=BTreeSet::new();
  for &method in &audio.methods{states.extend(self.analysis(method,&mut cache)?.states.iter().copied())}let state=unique(states.iter().copied().collect()).map_err(|_|format!("State members: {states:?}"))?;
  // The audio class's time query subtracts position from total duration. Require
  // the same state member as its RTTI-bound methods, and one unambiguous source
  // field in the nearby class implementation. No duration member is hard-coded.
  let low=audio.methods.iter().min().copied().ok_or_else(bad)?.saturating_sub(65536);
  let high=audio.methods.iter().max().copied().ok_or_else(bad)?.saturating_add(65536);
  let mut starts=BTreeSet::new();
  if self.bits==64{starts.extend(self.functions.iter().filter_map(|(start,_)|(*start>=low&&*start<high).then_some(*start)));}
  else{for s in &self.sections{if s.flags&EXEC==0{continue}let from=low.max(s.rva);let to=high.min(s.rva+s.raw_size);if from>=to{continue}
   let code=self.at(from,to-from)?;
   for(at,window)in code.windows(3).enumerate(){if window==[0x55,0x8b,0xec]{starts.insert(from+at);}}
   // Optimized x86 time queries can omit EBP. Include aligned entries after
   // compiler INT3 padding, then apply the same member/data-flow constraints.
   for at in 1..code.len(){if (from+at)%16==0&&code[at-1]==0xcc&&code[at]!=0xcc{starts.insert(from+at);}}
  }}
  let mut durations=BTreeSet::new();for start in starts{let Ok(a)=self.analysis(start,&mut cache)else{continue};if a.states.contains(&state){durations.extend(a.remaining.iter().copied());}}
  let duration=unique(durations.iter().copied().collect()).map_err(|_|format!("Duration members: {durations:?}"))?;
  let mut layouts=Vec::new();
  for name in INFOS{let tables=self.vtables(name)?;let prim=tables.iter().filter(|t|t.offset==0).collect::<Vec<_>>();if prim.is_empty(){continue}let prim=unique(prim)?;
   let mut fields=BTreeSet::new();for &method in &prim.methods{fields.extend(self.analysis(method,&mut cache)?.fields.iter().copied())}
   if fields.is_empty(){continue}let id_fields=fields.into_iter().collect::<Vec<_>>();
   for table in tables.iter().filter(|t|t.offset>0){for &method in &table.methods{let calls=self.analysis(method,&mut cache)?.calls.clone();
    for target in calls{if !self.executable(target,1){continue}let a=self.analysis(target,&mut cache)?;
     let positions=a.stores.iter().filter_map(|(k,v)|(*k==Origin::Position).then_some(*v)).collect::<Vec<_>>();
     let durations=a.stores.iter().filter_map(|(k,v)|(*k==Origin::Buffered).then_some(*v)).collect::<Vec<_>>();
     if positions.len()==1&&durations.len()==1&&positions[0]!=durations[0]{layouts.push(Layout{audio:audio.rva,info:prim.rva,secondary:table.rva,secondary_offset:table.offset,state,id_fields:id_fields.clone(),position:positions[0],buffered:durations[0],duration})}
    }
   }}
  }layouts.sort_by_key(|l|(l.info,l.position,l.duration));layouts.dedup();let count=layouts.len();unique(layouts).map_err(|_|format!("Clock/interface candidates: {count}"))
 }
 fn analysis<'a>(&self,rva:usize,cache:&'a mut BTreeMap<usize,Analysis>)->Result<&'a Analysis>{if !cache.contains_key(&rva){cache.insert(rva,self.analyze(&self.instructions(rva)?));}cache.get(&rva).ok_or_else(bad)}
 fn analyze(&self,instructions:&[Instruction])->Analysis{
  let mut a=Analysis::default();let mut flow=Flow::new(self.bits);let mut compared=BTreeSet::new();let mut last_load=None;
  for(index,i)in instructions.iter().enumerate(){
   if i.mnemonic()==M::Cmp&&i.op0_kind()==O::Memory&&i.memory_size().size()==4&&i.try_immediate(1).ok()==Some(1){if let Some(Value(Origin::This,d))=flow.address(i,self){if (1..1024).contains(&d){a.states.insert(d as usize);}}}
   if i.mnemonic()==M::Cmp&&i.op0_kind()==O::Register&&i.try_immediate(1).ok()==Some(1){if let Some(Value(Origin::Member,d))=flow.value(i,0,self){if (1..1024).contains(&d){a.states.insert(d as usize);}}}
   if i.mnemonic()==M::Movsd&&i.op0_kind()==O::Register&&i.op1_kind()==O::Memory{last_load=match flow.address(i,self){Some(Value(Origin::This,d))if (1..1024).contains(&d)=>Some((i.op0_register().full_register(),d as usize,index)),_=>None};}
   if i.mnemonic()==M::Subsd&&i.op0_kind()==O::Register{if let Some((r,field,at))=last_load{if index-at<=3&&i.op0_register().full_register()==r{a.remaining.insert(field);}}}
   if matches!(i.mnemonic(),M::Ucomisd|M::Comisd)&&i.op1_kind()==O::Memory{if let(Some(Value(origin,_)),Some(Value(Origin::Global,rva)))=(flow.value(i,0,self),flow.address(i,self)){
    if matches!(origin,Origin::Position|Origin::Buffered)&&rva>=0&&self.writable(rva as usize,8){compared.insert((origin,rva as usize));}
   }}
   if i.mnemonic()==M::Movsd&&i.op0_kind()==O::Memory{if let(Some(Value(origin,_)),Some(Value(Origin::Global,rva)))=(flow.value(i,1,self),flow.address(i,self)){
    if rva>=0&&compared.contains(&(origin,rva as usize)){a.stores.insert((origin,rva as usize));}
   }}
   let m=i.mnemonic();
   match m{
    M::Push=>{let v=flow.value(i,0,self);if let Some(Value(Origin::Stack,s))=flow.get(R::RSP){flow.set(R::RSP,Some(Value(Origin::Stack,s-self.ptr()as i64)));flow.stack.insert(s-self.ptr()as i64,v);}},
    M::Pop=>{if let Some(Value(Origin::Stack,s))=flow.get(R::RSP){flow.write(i,0,flow.stack.get(&s).copied().flatten(),self);flow.set(R::RSP,Some(Value(Origin::Stack,s+self.ptr()as i64)));}},
    M::Mov|M::Movsd|M::Movaps|M::Movapd|M::Movq=>{let v=flow.value(i,1,self);flow.write(i,0,v,self);},
    M::Lea=>{let v=flow.address(i,self);if let Some(Value(Origin::This,d))=v{if (1..1024).contains(&d){a.fields.insert(d as usize);}}flow.write(i,0,v,self);},
    M::Add|M::Sub if i.op0_kind()==O::Register=>{let v=flow.value(i,0,self).zip(i.try_immediate(1).ok()).and_then(|(Value(o,n),d)|n.checked_add(if m==M::Add{d as i64}else{-(d as i64)}).map(|n|Value(o,n)));
     if let Some(Value(Origin::This,d))=v{if (1..1024).contains(&d){a.fields.insert(d as usize);}}flow.write(i,0,v,self);
    },
    M::And if i.op0_kind()==O::Register=>{let v=(i.op0_register().full_register()==R::RSP).then_some(Value(Origin::Stack,-0x100000));flow.write(i,0,v,self);},
    M::Call=>{if matches!(i.op0_kind(),O::NearBranch32|O::NearBranch64){if let Ok(rva)=self.rva(i.near_branch_target()){a.calls.insert(rva);}}
     for r in [R::RAX,R::RCX,R::RDX]{flow.set(r,None)}
     if self.bits==64{for r in [R::R8,R::R9,R::R10,R::R11]{flow.set(r,None)}}
     for n in 0..if self.bits==64{6}else{8}{flow.set(R::try_from(R::XMM0 as usize+n).unwrap(),None)}
    },
    M::Xor|M::Xorps|M::Xorpd=>flow.write(i,0,None,self),
    _=>{},
   }
  }a
 }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord)]enum Origin{This,Member,Stack,Global,Constant,Position,Buffered}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]struct Value(Origin,i64);
#[derive(Default)]struct Analysis{states:BTreeSet<usize>,fields:BTreeSet<usize>,calls:BTreeSet<usize>,stores:BTreeSet<(Origin,usize)>,remaining:BTreeSet<usize>}
struct Flow{registers:BTreeMap<R,Value>,stack:BTreeMap<i64,Option<Value>>}
impl Flow{
 fn new(bits:u32)->Self{let mut f=Self{registers:BTreeMap::new(),stack:BTreeMap::new()};for(r,v)in [(R::RCX,Value(Origin::This,0)),(R::RSP,Value(Origin::Stack,0)),(R::XMM1,Value(Origin::Position,0)),(R::XMM2,Value(Origin::Buffered,0))]{f.set(r,Some(v));}
  if bits==32{f.stack.insert(8,Some(Value(Origin::Position,0)));f.stack.insert(16,Some(Value(Origin::Buffered,0)));}f
 }
 fn get(&self,r:R)->Option<Value>{self.registers.get(&r.full_register()).copied()}
 fn set(&mut self,r:R,v:Option<Value>){let r=r.full_register();if let Some(v)=v{self.registers.insert(r,v);}else{self.registers.remove(&r);}}
 fn address(&self,i:&Instruction,image:&Image)->Option<Value>{
  if i.memory_index()!=R::None||matches!(i.memory_segment(),R::FS|R::GS){return None}
  if i.is_ip_rel_memory_operand(){return image.rva(i.ip_rel_memory_address()).ok().map(|r|Value(Origin::Global,r as i64))}
  if i.memory_base()==R::None{return image.rva(i.memory_displacement64()).ok().map(|r|Value(Origin::Global,r as i64))}
  let Value(o,n)=self.get(i.memory_base())?;let disp=if image.bits==32{i.memory_displacement32()as i32 as i64}else{i.memory_displacement64()as i64};n.checked_add(disp).map(|n|Value(o,n))
 }
 fn value(&self,i:&Instruction,op:u32,image:&Image)->Option<Value>{match i.op_kind(op){O::Register=>self.get(i.op_register(op)),O::Memory=>match self.address(i,image)?{Value(Origin::Stack,n)=>self.stack.get(&n).copied().flatten(),Value(Origin::This,n)=>Some(Value(Origin::Member,n)),_=>None},_=>i.try_immediate(op).ok().map(|n|Value(Origin::Constant,n as i64))}}
 fn write(&mut self,i:&Instruction,op:u32,v:Option<Value>,image:&Image){match i.op_kind(op){O::Register=>self.set(i.op_register(op),v),O::Memory=>{if let Some(Value(Origin::Stack,n))=self.address(i,image){self.stack.insert(n,v);}},_=>{}}}
}

#[cfg(test)]mod tests{
 use super::*;
 fn image(bits:u32)->Image{Image{file:vec![0;0x4000],base:if bits==64{0x180000000}else{0x10000000},bits,size:0x4000,timestamp:0,sections:vec![Section{rva:0,virtual_size:0x2000,raw:0,raw_size:0x2000,flags:READ|EXEC},Section{rva:0x2000,virtual_size:0x2000,raw:0x2000,raw_size:0x2000,flags:READ|WRITE}],functions:vec![]}}
 fn decode(im:&Image,code:&[u8])->Vec<Instruction>{Decoder::with_ip(im.bits,code,im.base+0x100,DecoderOptions::NONE).into_iter().collect()}
 #[test]fn member_locations_follow_this_through_stack_and_register_aliases(){
  let im=image(64);let code=[0x48,0x89,0x4c,0x24,0x08,0x55,0x48,0x83,0xec,0x20,0x48,0x8d,0x6c,0x24,0x10,0x48,0x8b,0x45,0x20,0x83,0x78,0x74,0x01,0x48,0x8d,0x50,0x28,0xc3];
  let a=im.analyze(&decode(&im,&code));assert_eq!(a.states,BTreeSet::from([0x74]));assert_eq!(a.fields,BTreeSet::from([0x28]));
  let im=image(32);let a=im.analyze(&decode(&im,&[0x55,0x8b,0xec,0x83,0xe4,0xf8,0x8b,0xf9,0x83,0x7f,0x44,0x01,0x8d,0x4f,0x1c,0xc3]));assert_eq!(a.states,BTreeSet::from([0x44]));assert_eq!(a.fields,BTreeSet::from([0x1c]));
 }
 #[test]fn progress_and_duration_follow_arguments_even_when_addresses_are_reversed(){
  let im=image(64);let mut code=vec![0x0f,0x28,0xf9,0x0f,0x28,0xf2];
  for(op,reg,rva)in [(0x2e,1,0x2020usize),(0x2e,2,0x2010),(0x11,7,0x2020),(0x11,6,0x2010)]{let prefix=if op==0x2e{0x66}else{0xf2};let next=0x100+code.len()+8;code.extend([prefix,0x0f,op,0x05+8*reg]);code.extend(((rva as i32)-(next as i32)).to_le_bytes());}
  code.push(0xc3);let a=im.analyze(&decode(&im,&code));assert_eq!(a.stores,BTreeSet::from([(Origin::Position,0x2020),(Origin::Buffered,0x2010)]));
  // A compare/store alone is insufficient if its source is unrelated to a time argument.
  let mut changed=code;changed[2]=0xfb;assert!(!im.analyze(&decode(&im,&changed)).stores.contains(&(Origin::Position,0x2020)));
 }
 #[test]fn x86_absolute_operands_use_the_same_argument_proof(){
  let im=image(32);let code=[0x55,0x8b,0xec,0xf2,0x0f,0x10,0x4d,0x0c,0x66,0x0f,0x2e,0x0d,0x20,0x20,0x00,0x10,0xf2,0x0f,0x11,0x0d,0x20,0x20,0x00,0x10,0xc3];
  assert_eq!(im.analyze(&decode(&im,&code)).stores,BTreeSet::from([(Origin::Position,0x2020)]));
 }
 #[test]fn total_duration_is_an_object_member_not_the_buffer_callback_argument(){
  for bits in [32,64]{let im=image(bits);let prefix=if bits==64{vec![0x83,0x79,0x74,0x01]}else{vec![0x83,0x79,0x44,0x01]};
   let mut code=prefix;code.extend([0xf2,0x0f,0x10,0x81,0xb8,0,0,0,0xf2,0x0f,0x5c,0xc1,0xc3]);
   let a=im.analyze(&decode(&im,&code));assert_eq!(a.remaining,BTreeSet::from([0xb8]));assert!(a.stores.is_empty());
   let length=code.len();code[length-4]=0x58;assert!(im.analyze(&decode(&im,&code)).remaining.is_empty());
  }
 }
 #[test]fn image_parser_rejects_truncation_unsupported_architectures_and_overlapping_sections(){
  for bits in [32,64]{let mut b=vec![0u8;0x400];b[..2].copy_from_slice(b"MZ");b[0x3c]=0x80;b[0x80..0x84].copy_from_slice(b"PE\0\0");
   let(machine,magic,optional)=(if bits==64{0x8664u16}else{0x14c},if bits==64{0x20bu16}else{0x10b},if bits==64{240u16}else{224});
   b[0x84..0x86].copy_from_slice(&machine.to_le_bytes());b[0x86]=1;b[0x94..0x96].copy_from_slice(&optional.to_le_bytes());b[0x98..0x9a].copy_from_slice(&magic.to_le_bytes());b[0xd0..0xd4].copy_from_slice(&0x2000u32.to_le_bytes());
   let section=0x98+optional as usize;b[section+8..section+12].copy_from_slice(&0x1000u32.to_le_bytes());b[section+12..section+16].copy_from_slice(&0x1000u32.to_le_bytes());
   assert!(Image::parse(b.clone()).is_ok());for end in [0,2,0x3c,0x84,section+15]{assert!(Image::parse(b[..end].to_vec()).is_err());}
   b[0x86]=2;let first=b[section..section+40].to_vec();b[section+40..section+80].copy_from_slice(&first);assert!(Image::parse(b.clone()).is_err());
   b[0x86]=1;b[0x84..0x86].copy_from_slice(&0xaa64u16.to_le_bytes());assert!(Image::parse(b).is_err());
  }
 }
 #[test]fn read_only_code_and_unproven_stores_cannot_become_clocks(){let im=image(64);let code=[0xf2,0x0f,0x11,0x0d,0x18,0x1f,0,0,0xc3];assert!(im.analyze(&decode(&im,&code)).stores.is_empty());assert!(Image::parse(vec![0;4096]).is_err());}
}
