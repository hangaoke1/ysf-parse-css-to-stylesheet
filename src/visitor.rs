use std::{
  cell::RefCell,
  collections::HashMap,
  hash::{Hash, Hasher},
  rc::Rc,
};

use ego_tree::{NodeId, NodeMut, NodeRef, Tree};
use html5ever::{tendril::StrTendril, Attribute};
use lightningcss::{stylesheet::PrinterOptions, traits::ToCss};
use swc_common::{Span, DUMMY_SP};
use swc_ecma_ast::{
  Callee, ClassDecl, ClassMember, DefaultDecl, ExportDefaultDecl, ExportDefaultExpr, Expr, FnDecl,
  Function, Ident, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement,
  JSXElementChild, JSXElementName, JSXExpr, KeyValueProp, Lit, MemberProp, Program, Prop, PropName,
  PropOrSpread, Stmt, Str, JSXFragment, ImportDecl, ImportSpecifier,
};
use swc_ecma_visit::{
  noop_visit_mut_type, noop_visit_type, Visit, VisitMut, VisitMutWith, VisitWith,
};

use crate::{
  scraper::{Element, Fragment, Node},
  style_parser::StyleDeclaration,
  utils::{create_qualname, is_starts_with_uppercase, recursion_jsx_member},
};

#[derive(Eq, Clone, Debug)]
pub struct SpanKey(Span);

impl PartialEq for SpanKey {
  fn eq(&self, other: &Self) -> bool {
    self.0.lo == other.0.lo && self.0.hi == other.0.hi
  }
}

impl Hash for SpanKey {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.lo.hash(state);
    self.0.hi.hash(state);
  }
}

pub type JSXRecord = HashMap<SpanKey, NodeId>;

fn recursion_sub_tree<'a>(node: &NodeRef<Node>, current: &mut NodeMut<'a, Node>) {
  for child in node.children() {
    let mut tree_node = current.append(child.value().clone());
    recursion_sub_tree(&child, &mut tree_node);
  }
}

pub struct JSXVisitor<'a> {
  pub tree: &'a mut Tree<Node>,
  pub module: &'a Program,
  pub jsx_record: &'a mut JSXRecord,
  pub taro_components: &'a [String],
  pub root_node: Option<NodeId>,
  pub current_node: Option<NodeId>,
}

impl<'a> JSXVisitor<'a> {
  pub fn new(tree: &'a mut Tree<Node>, module: &'a Program, jsx_record: &'a mut JSXRecord, taro_components: &'a [String]) -> Self {
    JSXVisitor {
      tree,
      module,
      jsx_record,
      taro_components,
      root_node: None,
      current_node: None,
    }
  }
  fn create_element(&mut self, jsx_element: &JSXElement) -> Node {
    let name = match &jsx_element.opening.name {
      JSXElementName::Ident(ident) => ident.sym.to_string(),
      JSXElementName::JSXMemberExpr(expr) => recursion_jsx_member(expr),
      JSXElementName::JSXNamespacedName(namespaced_name) => {
        format!(
          "{}:{}",
          namespaced_name.ns.sym.to_string(),
          namespaced_name.name.sym.to_string()
        )
      }
    };
    let qual_name = create_qualname(name.as_str());
    let mut attributes = Vec::new();
    for attr in &jsx_element.opening.attrs {
      if let JSXAttrOrSpread::JSXAttr(attr) = attr {
        let name = match &attr.name {
          JSXAttrName::Ident(ident) => ident.sym.to_string(),
          JSXAttrName::JSXNamespacedName(namespaced_name) => {
            format!(
              "{}:{}",
              namespaced_name.ns.sym.to_string(),
              namespaced_name.name.sym.to_string()
            )
          }
        };
        let value = match &attr.value {
          Some(value) => match value {
            JSXAttrValue::Lit(lit) => match lit {
              Lit::Str(str) => str.value.to_string(),
              Lit::Num(num) => num.value.to_string(),
              Lit::Bool(bool) => bool.value.to_string(),
              Lit::Null(_) => "null".to_string(),
              Lit::BigInt(bigint) => bigint.value.to_string(),
              Lit::Regex(regex) => regex.exp.to_string(),
              Lit::JSXText(text) => text.value.to_string(),
            },
            JSXAttrValue::JSXExprContainer(expr_container) => match &expr_container.expr {
              JSXExpr::JSXEmptyExpr(_) => "{{}}".to_string(),
              JSXExpr::Expr(expr) => match &**expr {
                Expr::Lit(lit) => match lit {
                  Lit::Str(str) => str.value.to_string(),
                  Lit::Num(num) => num.value.to_string(),
                  Lit::Bool(bool) => bool.value.to_string(),
                  Lit::Null(_) => "null".to_string(),
                  Lit::BigInt(bigint) => bigint.value.to_string(),
                  Lit::Regex(regex) => regex.exp.to_string(),
                  Lit::JSXText(text) => text.value.to_string(),
                },
                _ => "".to_string(),
              },
            },
            JSXAttrValue::JSXElement(_) => "".to_string(),
            JSXAttrValue::JSXFragment(_) => "".to_string(),
          },
          None => "".to_string(),
        };
        attributes.push(Attribute {
          name: create_qualname(name.as_str()),
          value: StrTendril::from(value),
        });
      }
    }
    Node::Element(Element::new(qual_name, attributes))
  }

  fn create_fragment(&mut self) -> Node {
    Node::Fragment(Fragment::new(Some(create_qualname("__Fragment__"))))
  }
}

impl<'a> Visit for JSXVisitor<'a> {
  noop_visit_type!();

  fn visit_jsx_element(&mut self, jsx: &JSXElement) {
    if self.root_node.is_none() {
      let node = self.create_element(jsx);
      let mut root = self.tree.root_mut();
      self.root_node = Some(root.id());
      let current = root.append(node);
      self.current_node = Some(current.id());
      self.jsx_record.insert(SpanKey(jsx.span), current.id());
    }
    jsx.visit_children_with(self)
  }

  fn visit_jsx_fragment(&mut self, n: &JSXFragment) {
    if self.root_node.is_none() {
      let node = self.create_fragment();
      let mut root = self.tree.root_mut();
      self.root_node = Some(root.id());
      let current = root.append(node);
      self.current_node = Some(current.id());
      self.jsx_record.insert(SpanKey(n.span), current.id());
    }
    n.visit_children_with(self)
  }

  fn visit_jsx_element_children(&mut self, n: &[JSXElementChild]) {
    let mut nodes = vec![];
    let mut elements: Vec<&JSXElementChild> = vec![];
    for child in n.iter() {
      match child {
        JSXElementChild::JSXElement(element) => {
          if let JSXElementName::Ident(ident) = &element.opening.name {
            let name = ident.sym.to_string();
            if is_starts_with_uppercase(name.as_str()) && !self.taro_components.contains(&name) {
              let mut visitor = JSXFragmentVisitor::new(
                self.module,
                self.jsx_record,
                self.taro_components,
                name.as_str(),
                SearchType::Normal,
              );
              self.module.visit_with(&mut visitor);
              if let Some(current_node) = self.current_node {
                if let Some(mut current) = self.tree.get_mut(current_node) {
                  // 将 Fragment 的子节点添加到当前节点
                  recursion_sub_tree(&visitor.tree.root(), &mut current);
                }
              }
            } else {
              let node = self.create_element(element);
              if let Some(current_node) = self.current_node {
                if let Some(mut current) = self.tree.get_mut(current_node) {
                  let tree_node = current.append(node);
                  nodes.push(tree_node.id());
                  elements.push(child);
                  self
                    .jsx_record
                    .insert(SpanKey(element.span), tree_node.id());
                }
              }
            }
          } else {
            let node = self.create_element(element);
            if let Some(current_node) = self.current_node {
              if let Some(mut current) = self.tree.get_mut(current_node) {
                let tree_node = current.append(node);
                nodes.push(tree_node.id());
                elements.push(child);
                self
                  .jsx_record
                  .insert(SpanKey(element.span), tree_node.id());
              }
            }
          }
        }
        JSXElementChild::JSXFragment(fragment) => {
          let node = self.create_fragment();
          if let Some(current_node) = self.current_node {
            if let Some(mut current) = self.tree.get_mut(current_node) {
              let tree_node = current.append(node);
              nodes.push(tree_node.id());
              elements.push(child);
              self
                .jsx_record
                .insert(SpanKey(fragment.span), tree_node.id());
            }
          }
        }
        // 找到函数调用中的 JSX
        JSXElementChild::JSXExprContainer(expr) => {
          match &expr.expr {
            JSXExpr::JSXEmptyExpr(_) => {}
            JSXExpr::Expr(expr) => {
              match &**expr {
                Expr::Call(call_expr) => {
                  match &call_expr.callee {
                    Callee::Expr(expr) => {
                      match &**expr {
                        Expr::Ident(ident) => {
                          let name = ident.sym.to_string();
                          let mut visitor = JSXFragmentVisitor::new(
                            self.module,
                            self.jsx_record,
                            self.taro_components,
                            name.as_str(),
                            SearchType::Normal,
                          );
                          self.module.visit_with(&mut visitor);
                          if let Some(current_node) = self.current_node {
                            if let Some(mut current) = self.tree.get_mut(current_node) {
                              // 将 Fragment 的子节点添加到当前节点
                              recursion_sub_tree(&visitor.tree.root(), &mut current);
                            }
                          }
                        }
                        Expr::Member(member_expr) => {
                          if let Expr::This(_) = &*member_expr.obj {
                            match &member_expr.prop {
                              MemberProp::Ident(ident) => {
                                let name = ident.sym.to_string();
                                let mut visitor = JSXFragmentVisitor::new(
                                  self.module,
                                  self.jsx_record,
                                  self.taro_components,
                                  name.as_str(),
                                  SearchType::Class,
                                );
                                self.module.visit_with(&mut visitor);
                                if let Some(current_node) = self.current_node {
                                  if let Some(mut current) = self.tree.get_mut(current_node) {
                                    // 将 Fragment 的子节点添加到当前节点
                                    recursion_sub_tree(&visitor.tree.root(), &mut current);
                                  }
                                }
                              }
                              _ => {}
                            }
                          }
                        }
                        _ => {}
                      }
                    }
                    _ => {}
                  }
                }
                _ => {}
              }
            }
          }
        }
        _ => {}
      }
    }
    for (index, element) in elements.iter().enumerate() {
      let mut visitor = JSXVisitor::new(self.tree, self.module, self.jsx_record, self.taro_components);
      visitor.current_node = Some(nodes[index]);
      visitor.root_node = self.root_node;
      element.visit_with(&mut visitor);
    }
  }
}

#[derive(PartialEq)]
pub enum SearchType {
  Normal,
  Class,
}

pub struct JSXFragmentVisitor<'a> {
  pub module: &'a Program,
  pub tree: Tree<Node>,
  pub jsx_record: &'a mut JSXRecord,
  pub taro_components: &'a [String],
  pub search_fn: &'a str,
  pub search_type: SearchType,
}

impl<'a> JSXFragmentVisitor<'a> {
  pub fn new(
    module: &'a Program,
    jsx_record: &'a mut JSXRecord,
    taro_components: &'a [String],
    search_fn: &'a str,
    search_type: SearchType,
  ) -> Self {
    JSXFragmentVisitor {
      module,
      jsx_record,
      taro_components,
      tree: Tree::new(Node::Fragment(Fragment::new(Some(create_qualname(
        search_fn,
      ))))),
      search_fn,
      search_type,
    }
  }
}

impl<'a> Visit for JSXFragmentVisitor<'a> {
  noop_visit_type!();

  fn visit_fn_decl(&mut self, n: &FnDecl) {
    if n.ident.sym.to_string() == self.search_fn && self.search_type == SearchType::Normal {
      match &*n.function {
        Function {
          body: Some(body), ..
        } => {
          for stmt in &body.stmts {
            match stmt {
              Stmt::Return(return_stmt) => {
                let mut jsx_visitor = JSXVisitor::new(&mut self.tree, self.module, self.jsx_record, self.taro_components);
                return_stmt.visit_with(&mut jsx_visitor);
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
  }

  fn visit_class_method(&mut self, n: &swc_ecma_ast::ClassMethod) {
    if self.search_type == SearchType::Class {
      match &n.key {
        PropName::Ident(ident) => {
          if ident.sym.to_string() == self.search_fn {
            match &*n.function {
              Function {
                body: Some(body), ..
              } => {
                for stmt in &body.stmts {
                  match stmt {
                    Stmt::Return(return_stmt) => {
                      let mut jsx_visitor =
                        JSXVisitor::new(&mut self.tree, self.module, self.jsx_record, self.taro_components);
                      return_stmt.visit_with(&mut jsx_visitor);
                    }
                    _ => {}
                  }
                }
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
  }
}

pub struct CollectVisitor {
  pub export_default_name: Option<String>,
  pub taro_components: Vec<String>,
}

impl CollectVisitor {
  pub fn new() -> Self {
    CollectVisitor {
      export_default_name: None,
      taro_components: vec![],
    }
  }
}

impl Visit for CollectVisitor {
  fn visit_export_default_expr(&mut self, n: &ExportDefaultExpr) {
    match &*n.expr {
      Expr::Ident(ident) => {
        if self.export_default_name.is_none() {
          self.export_default_name = Some(ident.sym.to_string());
        }
      }
      _ => {}
    }
  }

  fn visit_import_decl(&mut self, n: &ImportDecl) {
    if n.src.value.to_string().starts_with("@tarojs/components") {
      for specifier in &n.specifiers {
        match specifier {
          ImportSpecifier::Named(named_specifier) => {
            self.taro_components.push(named_specifier.local.sym.to_string())
          }
          _ => {}
        }
      }
    }
  }
}

pub struct AstVisitor<'a> {
  pub export_default_name: &'a Option<String>,
  pub taro_components: &'a [String],
  pub module: &'a Program,
  pub tree: &'a mut Tree<Node>,
  pub jsx_record: &'a mut JSXRecord,
}

impl<'a> AstVisitor<'a> {
  pub fn new(module: &'a Program, tree: &'a mut Tree<Node>, jsx_record: &'a mut JSXRecord, export_default_name: &'a Option<String>, taro_components: &'a [String]) -> Self {
    AstVisitor {
      export_default_name,
      taro_components,
      module,
      tree,
      jsx_record,
    }
  }
}

impl<'a> Visit for AstVisitor<'a> {
  noop_visit_type!();

  fn visit_fn_decl(&mut self, n: &FnDecl) {
    match &self.export_default_name {
      Some(name) => {
        if n.ident.sym.to_string() == name.as_str() {
          match &*n.function {
            Function {
              body: Some(body), ..
            } => {
              for stmt in &body.stmts {
                match stmt {
                  Stmt::Return(return_stmt) => {
                    let mut jsx_visitor = JSXVisitor::new(self.tree, self.module, self.jsx_record, self.taro_components);
                    return_stmt.visit_with(&mut jsx_visitor);
                  }
                  _ => {}
                }
              }
            }
            _ => {}
          }
        }
      }
      None => {}
    }
  }

  fn visit_class_decl(&mut self, n: &ClassDecl) {
    match &self.export_default_name {
      Some(name) => {
        if n.ident.sym.to_string() == name.as_str() {
          for member in &n.class.body {
            match member {
              ClassMember::Method(method) => match &method.key {
                PropName::Ident(ident) => {
                  if ident.sym.to_string() == "render" {
                    match &*method.function {
                      Function {
                        body: Some(body), ..
                      } => {
                        for stmt in &body.stmts {
                          match stmt {
                            Stmt::Return(return_stmt) => {
                              let mut jsx_visitor =
                                JSXVisitor::new(self.tree, self.module, self.jsx_record, self.taro_components);
                              return_stmt.visit_with(&mut jsx_visitor);
                            }
                            _ => {}
                          }
                        }
                      }
                      _ => {}
                    }
                  }
                }
                _ => {}
              },
              _ => {}
            }
          }
        }
      }
      None => {}
    }
  }

  fn visit_export_default_decl(&mut self, n: &ExportDefaultDecl) {
    match &n.decl {
      DefaultDecl::Fn(n) => match &*n.function {
        Function {
          body: Some(body), ..
        } => {
          for stmt in &body.stmts {
            match stmt {
              Stmt::Return(return_stmt) => {
                let mut jsx_visitor = JSXVisitor::new(self.tree, self.module, self.jsx_record, self.taro_components);
                return_stmt.visit_with(&mut jsx_visitor);
              }
              _ => {}
            }
          }
        }
        _ => {}
      },
      DefaultDecl::Class(n) => {
        for member in &n.class.body {
          match member {
            ClassMember::Method(method) => match &method.key {
              PropName::Ident(ident) => {
                if ident.sym.to_string() == "render" {
                  match &*method.function {
                    Function {
                      body: Some(body), ..
                    } => {
                      for stmt in &body.stmts {
                        match stmt {
                          Stmt::Return(return_stmt) => {
                            let mut jsx_visitor =
                              JSXVisitor::new(self.tree, self.module, self.jsx_record, self.taro_components);
                            return_stmt.visit_with(&mut jsx_visitor);
                          }
                          _ => {}
                        }
                      }
                    }
                    _ => {}
                  }
                }
              }
              _ => {}
            },
            _ => {}
          }
        }
      }
      _ => {}
    }
  }
}

pub struct AstMutVisitor<'a> {
  pub jsx_record: Rc<RefCell<JSXRecord>>,
  pub style_record: Rc<RefCell<HashMap<NodeId, StyleDeclaration<'a>>>>,
}

impl<'a> AstMutVisitor<'a> {
  pub fn new(
    jsx_record: Rc<RefCell<JSXRecord>>,
    style_record: Rc<RefCell<HashMap<NodeId, StyleDeclaration<'a>>>>,
  ) -> Self {
    AstMutVisitor {
      jsx_record,
      style_record,
    }
  }
}

impl<'a> VisitMut for AstMutVisitor<'a> {
  noop_visit_mut_type!();

  fn visit_mut_jsx_element(&mut self, n: &mut JSXElement) {
    let span_key = SpanKey(n.span);
    if let Some(node_id) = self.jsx_record.borrow().get(&span_key) {
      // 将 style_record 中的样式添加到 JSXElement 的 style 属性中
      let style_record = self.style_record.borrow();
      let attrs = &mut n.opening.attrs;
      let mut has_style = false;
      let mut has_empty_style = false;
      for attr in attrs {
        if let JSXAttrOrSpread::JSXAttr(attr) = attr {
          if let JSXAttrName::Ident(ident) = &attr.name {
            if ident.sym.to_string() == "style" {
              has_style = true;
              // 只支持值为字符串、对象形式的 style
              match &mut attr.value {
                Some(value) => {
                  match value {
                    JSXAttrValue::Lit(lit) => {
                      match lit {
                        Lit::Str(str) => {
                          // 将 style 属性的值转换为对象形式
                          let mut properties = HashMap::new();
                          let style = str.value.to_string();
                          let style = style
                            .split(";")
                            .map(|s| s.to_owned())
                            .collect::<Vec<String>>();
                          if let Some(style_declaration) = style_record.get(node_id) {
                            for declaration in style_declaration.declaration.declarations.iter() {
                              let property_id = declaration
                                .property_id()
                                .to_css_string(PrinterOptions::default())
                                .unwrap();
                              let property_value = declaration
                                .value_to_css_string(PrinterOptions::default())
                                .unwrap();
                              properties.insert(property_id, property_value);
                            }
                          }
                          for property in style.iter() {
                            let property = property
                              .split(":")
                              .map(|s| s.to_owned())
                              .collect::<Vec<String>>();
                            if property.len() == 2 {
                              properties.insert(property[0].clone(), property[1].clone());
                            }
                          }
                          let mut style = String::new();
                          for (property_id, property_value) in properties.iter() {
                            style.push_str(property_id.as_str());
                            style.push_str(":");
                            style.push_str(property_value.as_str());
                            style.push_str(";");
                          }
                          attr.value = Some(JSXAttrValue::Lit(Lit::Str(Str {
                            span: DUMMY_SP,
                            value: style.into(),
                            raw: None,
                          })));
                        }
                        _ => {}
                      }
                    }
                    JSXAttrValue::JSXExprContainer(expr_container) => {
                      match &mut expr_container.expr {
                        JSXExpr::JSXEmptyExpr(_) => {
                          has_empty_style = true;
                          has_style = false;
                        }
                        JSXExpr::Expr(expr) => match &mut **expr {
                          Expr::Object(lit) => {
                            let mut properties = Vec::new();
                            if let Some(style_declaration) = style_record.get(node_id) {
                              for declaration in style_declaration.declaration.declarations.iter() {
                                let mut has_property = false;
                                for prop in lit.props.iter_mut() {
                                  match prop {
                                    PropOrSpread::Prop(prop) => match &**prop {
                                      Prop::KeyValue(key_value_prop) => match &key_value_prop.key {
                                        PropName::Ident(ident) => {
                                          let property_id = ident.sym.to_string();
                                          if property_id
                                            == declaration
                                              .property_id()
                                              .to_css_string(PrinterOptions::default())
                                              .unwrap()
                                          {
                                            has_property = true;
                                            break;
                                          }
                                        }
                                        _ => {}
                                      },
                                      _ => {}
                                    },
                                    PropOrSpread::Spread(_) => {}
                                  }
                                }
                                if !has_property {
                                  properties.push(declaration.clone());
                                }
                              }
                            }
                            for property in properties.iter() {
                              lit.props.push(PropOrSpread::Prop(Box::new(Prop::KeyValue(
                                KeyValueProp {
                                  key: PropName::Ident(Ident::new(
                                    property
                                      .property_id()
                                      .to_css_string(PrinterOptions::default())
                                      .unwrap()
                                      .into(),
                                    DUMMY_SP,
                                  )),
                                  value: property
                                    .value_to_css_string(PrinterOptions::default())
                                    .unwrap()
                                    .into(),
                                },
                              ))));
                            }
                          }
                          _ => {}
                        },
                      }
                    }
                    JSXAttrValue::JSXElement(_) => {}
                    JSXAttrValue::JSXFragment(_) => {}
                  }
                }
                None => {
                  has_empty_style = true;
                  has_style = false;
                }
              };
            }
          }
        }
      }

      if !has_style {
        if let Some(style_declaration) = style_record.get(node_id) {
          let mut properties = Vec::new();
          for declaration in style_declaration.declaration.declarations.iter() {
            properties.push(declaration.clone());
          }

          let mut style = String::new();
          for property in properties.iter() {
            let property_id = property
              .property_id()
              .to_css_string(PrinterOptions::default())
              .unwrap();
            let property_value = property
              .value_to_css_string(PrinterOptions::default())
              .unwrap();
            style.push_str(property_id.as_str());
            style.push_str(":");
            style.push_str(property_value.as_str());
            style.push_str(";");
          }
          if has_empty_style {
            for attr in &mut n.opening.attrs {
              if let JSXAttrOrSpread::JSXAttr(attr) = attr {
                if let JSXAttrName::Ident(ident) = &attr.name {
                  if ident.sym.to_string() == "style" {
                    attr.value = Some(JSXAttrValue::Lit(Lit::Str(Str {
                      span: DUMMY_SP,
                      value: style.clone().into(),
                      raw: None,
                    })));
                  }
                }
              }
            }
          } else {
            n.opening.attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
              span: DUMMY_SP,
              name: JSXAttrName::Ident(Ident::new("style".into(), DUMMY_SP)),
              value: Some(JSXAttrValue::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                value: style.into(),
                raw: None,
              }))),
            }));
          }
        }
      }
    }
    n.visit_mut_children_with(self);
  }
}
