use std::collections::BTreeSet;

use crate::{
    compiled_module::ModuleExport,
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::BindingCell,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, OBJECT_CONSTRUCTOR_PROPERTY,
            PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyUpdate,
            PropertyWritable,
        },
    },
    value::Value,
};

use super::PendingModule;

#[derive(Debug)]
enum ExportResolution {
    Found(BindingCell),
    NotFound,
    Ambiguous,
}

#[derive(Clone, Copy)]
enum NamespaceBindingKind {
    Eager,
    Deferred,
}

impl Context {
    pub(super) fn validate_indirect_exports(&mut self, graph: &[PendingModule]) -> Result<()> {
        for module_index in 0..graph.len() {
            let exports = graph
                .get(module_index)
                .ok_or_else(|| Error::runtime("module export owner is missing"))?
                .module
                .exports()
                .to_vec();
            for export in exports {
                match export {
                    ModuleExport::Indirect { export_name, .. }
                    | ModuleExport::Source { export_name, .. } => {
                        self.required_export_cell(graph, module_index, &export_name)?;
                    }
                    ModuleExport::Local { .. }
                    | ModuleExport::Namespace { .. }
                    | ModuleExport::DeferredNamespace { .. }
                    | ModuleExport::Star { .. } => {}
                }
            }
        }
        Ok(())
    }

    pub(super) fn initialize_module_namespaces(
        &mut self,
        graph: &mut [PendingModule],
    ) -> Result<()> {
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        for pending in graph {
            let (namespace, binding) = self.create_module_namespace_shell(constructor_key)?;
            let (deferred_namespace, deferred_binding) =
                self.create_module_namespace_shell(constructor_key)?;
            pending.namespace = Some(namespace);
            pending.namespace_binding = Some(binding);
            pending.deferred_namespace = Some(deferred_namespace);
            pending.deferred_namespace_binding = Some(deferred_binding);
        }
        Ok(())
    }

    fn create_module_namespace_shell(
        &mut self,
        constructor_key: PropertyKey,
    ) -> Result<(Value, BindingCell)> {
        let namespace = self.objects.create(
            Vec::new(),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(namespace_id) = namespace else {
            return Err(Error::runtime("module namespace is not an object"));
        };
        self.objects
            .set_prototype_value(namespace_id, &Value::Null)?;
        self.objects.mark_module_namespace(namespace_id)?;
        let namespace = Value::Object(namespace_id);
        let binding = BindingCell::new(namespace.clone(), false, crate::syntax::DeclKind::Const);
        Ok((namespace, binding))
    }

    pub(super) fn populate_module_namespaces(&mut self, graph: &[PendingModule]) -> Result<()> {
        for module_index in 0..graph.len() {
            let names = Self::module_export_names(graph, module_index, &mut BTreeSet::new())?;
            let module = graph
                .get(module_index)
                .ok_or_else(|| Error::runtime("module namespace owner is missing"))?;
            let namespaces = [
                (
                    module
                        .namespace
                        .clone()
                        .ok_or_else(|| Error::runtime("module namespace object is missing"))?,
                    "Module",
                ),
                (
                    module.deferred_namespace.clone().ok_or_else(|| {
                        Error::runtime("deferred module namespace object is missing")
                    })?,
                    "Deferred Module",
                ),
            ];
            for (namespace, tag) in namespaces {
                let Value::Object(namespace_id) = namespace else {
                    return Err(Error::runtime("module namespace value is not an object"));
                };
                self.populate_module_namespace(graph, module_index, namespace_id, &names, tag)?;
            }
        }
        Ok(())
    }

    fn populate_module_namespace(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        namespace_id: crate::value::ObjectId,
        names: &BTreeSet<String>,
        tag: &str,
    ) -> Result<()> {
        for name in names {
            let cell = match self.resolve_export(graph, module_index, name, &mut BTreeSet::new())? {
                ExportResolution::Found(cell) => cell,
                ExportResolution::Ambiguous => continue,
                ExportResolution::NotFound => {
                    return Err(Error::runtime(format!(
                        "module namespace export '{name}' could not be resolved"
                    )));
                }
            };
            let getter_name = format!("%module-namespace:{module_index}:{tag}:{name}%");
            let binding_name = name.clone();
            let getter = self.create_internal_host_function(getter_name, move |_call| {
                cell.value(&binding_name)
            })?;
            let key = self.intern_property_key(name)?;
            self.objects.define_property(
                namespace_id,
                key,
                name,
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(getter),
                    None,
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::No),
                )),
                self.limits.max_object_properties,
            )?;
        }
        self.define_module_namespace_to_string_tag(namespace_id, tag)?;
        self.objects.prevent_extensions(namespace_id)
    }

    fn define_module_namespace_to_string_tag(
        &mut self,
        namespace: crate::value::ObjectId,
        tag_name: &str,
    ) -> Result<()> {
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(tag_name)?;
        self.objects.define_property(
            namespace,
            PropertyKey::symbol(tag.id()),
            "[Symbol.toStringTag]",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn module_export_names(
        graph: &[PendingModule],
        module_index: usize,
        visiting: &mut BTreeSet<usize>,
    ) -> Result<BTreeSet<String>> {
        if !visiting.insert(module_index) {
            return Ok(BTreeSet::new());
        }
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module namespace owner is missing"))?;
        let mut names = BTreeSet::new();
        for export in module.module.exports() {
            match export {
                ModuleExport::Local { export_name, .. }
                | ModuleExport::Indirect { export_name, .. }
                | ModuleExport::Namespace { export_name, .. }
                | ModuleExport::DeferredNamespace { export_name, .. }
                | ModuleExport::Source { export_name, .. } => {
                    names.insert(export_name.clone());
                }
                ModuleExport::Star { request } => {
                    let dependency = module
                        .dependency_for_specifier(request)
                        .ok_or_else(|| Error::runtime("star namespace dependency is missing"))?;
                    let dependency_names = Self::module_export_names(graph, dependency, visiting)?;
                    names.extend(
                        dependency_names
                            .into_iter()
                            .filter(|name| name != "default"),
                    );
                }
            }
        }
        visiting.remove(&module_index);
        Ok(names)
    }

    pub(super) fn required_export_cell(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
    ) -> Result<BindingCell> {
        match self.resolve_export(graph, module_index, export_name, &mut BTreeSet::new())? {
            ExportResolution::Found(cell) => Ok(cell),
            ExportResolution::NotFound => {
                let module_name = graph
                    .get(module_index)
                    .map_or("<missing>", |module| module.name.as_str());
                Err(Error::exception(
                    crate::value::ErrorName::SyntaxError,
                    format!("module '{module_name}' does not export '{export_name}'"),
                ))
            }
            ExportResolution::Ambiguous => Err(Error::exception(
                crate::value::ErrorName::SyntaxError,
                format!("module export '{export_name}' is ambiguous"),
            )),
        }
    }

    fn resolve_export(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
        resolving: &mut BTreeSet<(usize, String)>,
    ) -> Result<ExportResolution> {
        let key = (module_index, export_name.to_owned());
        if !resolving.insert(key.clone()) {
            return Ok(ExportResolution::NotFound);
        }
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module export owner is missing"))?;
        let mut star_result = None;
        for export in module.module.exports() {
            match export {
                ModuleExport::Local {
                    export_name: candidate,
                    local_name,
                } if candidate == export_name => {
                    let atom = self.intern_atom(local_name)?;
                    let result = module
                        .scope
                        .as_ref()
                        .and_then(|scope| scope.get(atom))
                        .ok_or_else(|| Error::runtime("local module export is not declared"))
                        .and_then(|cell| cell.terminal_alias_target());
                    resolving.remove(&key);
                    return result.map(ExportResolution::Found);
                }
                ModuleExport::Indirect {
                    export_name: candidate,
                    import_name,
                    request,
                } if candidate == export_name => {
                    let dependency = module
                        .dependency_for_specifier(request)
                        .ok_or_else(|| Error::runtime("indirect export dependency is missing"))?;
                    let result = self.resolve_export(graph, dependency, import_name, resolving);
                    resolving.remove(&key);
                    return result;
                }
                ModuleExport::Namespace {
                    export_name: candidate,
                    request,
                } if candidate == export_name => {
                    let namespace = Self::exported_namespace_binding(
                        graph,
                        module_index,
                        request,
                        NamespaceBindingKind::Eager,
                    )?;
                    resolving.remove(&key);
                    return Ok(ExportResolution::Found(namespace));
                }
                ModuleExport::DeferredNamespace {
                    export_name: candidate,
                    request,
                } if candidate == export_name => {
                    let namespace = Self::exported_namespace_binding(
                        graph,
                        module_index,
                        request,
                        NamespaceBindingKind::Deferred,
                    )?;
                    resolving.remove(&key);
                    return Ok(ExportResolution::Found(namespace));
                }
                ModuleExport::Source {
                    export_name: candidate,
                    request,
                } if candidate == export_name => {
                    let dependency = module
                        .dependency_for_specifier(request)
                        .ok_or_else(|| Error::runtime("source export dependency is missing"))?;
                    let source = Self::required_module_source_binding(graph, dependency)?;
                    resolving.remove(&key);
                    return Ok(ExportResolution::Found(source));
                }
                ModuleExport::Star { request } if export_name != "default" => {
                    if let Some(resolution) = self.merge_star_export(
                        graph,
                        module_index,
                        export_name,
                        request,
                        &mut star_result,
                        resolving,
                    )? {
                        resolving.remove(&key);
                        return Ok(resolution);
                    }
                }
                ModuleExport::Local { .. }
                | ModuleExport::Indirect { .. }
                | ModuleExport::Namespace { .. }
                | ModuleExport::DeferredNamespace { .. }
                | ModuleExport::Source { .. }
                | ModuleExport::Star { .. } => {}
            }
        }
        resolving.remove(&key);
        Ok(star_result.map_or(ExportResolution::NotFound, ExportResolution::Found))
    }

    fn exported_namespace_binding(
        graph: &[PendingModule],
        module_index: usize,
        request: &str,
        kind: NamespaceBindingKind,
    ) -> Result<BindingCell> {
        let dependency = graph
            .get(module_index)
            .and_then(|module| module.dependency_for_specifier(request))
            .ok_or_else(|| Error::runtime("namespace export dependency is missing"))?;
        let pending = graph
            .get(dependency)
            .ok_or_else(|| Error::runtime("namespace export module is missing"))?;
        match kind {
            NamespaceBindingKind::Eager => pending.namespace_binding.clone(),
            NamespaceBindingKind::Deferred => pending.deferred_namespace_binding.clone(),
        }
        .ok_or_else(|| Error::runtime("exported module namespace binding is missing"))
    }

    fn merge_star_export(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
        request: &str,
        star_result: &mut Option<BindingCell>,
        resolving: &mut BTreeSet<(usize, String)>,
    ) -> Result<Option<ExportResolution>> {
        let dependency = graph
            .get(module_index)
            .and_then(|module| module.dependency_for_specifier(request))
            .ok_or_else(|| Error::runtime("star export dependency is missing"))?;
        match self.resolve_export(graph, dependency, export_name, resolving)? {
            ExportResolution::Found(cell) => {
                if star_result
                    .as_ref()
                    .is_some_and(|existing| !existing.same_cell(&cell))
                {
                    return Ok(Some(ExportResolution::Ambiguous));
                }
                *star_result = Some(cell);
                Ok(None)
            }
            ExportResolution::Ambiguous => Ok(Some(ExportResolution::Ambiguous)),
            ExportResolution::NotFound => Ok(None),
        }
    }
}
