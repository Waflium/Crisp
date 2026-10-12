use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::commands::create_compilation_context;
use crate::commands::functions::CommandFunction;
use crate::server::packs::resources::MultiPackResourceManager;
use crate::tags::{TagLoader, identifier_order};
use crate::util::java_collections::{JavaHashMap, identifier_hash};

const FUNCTION_DIRECTORY: &str = "function";
const FUNCTION_EXTENSION: &str = ".mcfunction";
const TAG_DIRECTORY: &str = "tags/function";

static NEXT_WORKER: AtomicUsize = AtomicUsize::new(2);

pub struct ServerFunctionLibrary {
    functions: JavaHashMap<Arc<CommandFunction>>,
    tags: JavaHashMap<Vec<Arc<CommandFunction>>>,
}

impl Default for ServerFunctionLibrary {
    fn default() -> ServerFunctionLibrary {
        ServerFunctionLibrary { functions: JavaHashMap::new(identifier_hash), tags: JavaHashMap::new(identifier_hash) }
    }
}

pub(crate) fn worker_name() -> String {
    format!("Worker-Main-{}", NEXT_WORKER.fetch_add(1, Ordering::Relaxed))
}

fn read_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    let mut rest = text.as_ref();
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(end) => {
                lines.push(rest[..end].to_string());
                let skip = if rest[end..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[end + skip..];
            }
            None => {
                lines.push(rest.to_string());
                break;
            }
        }
    }
    lines
}

impl ServerFunctionLibrary {
    pub fn get(&self, id: &str) -> Option<Arc<CommandFunction>> {
        self.functions.get(id).cloned()
    }

    pub fn get_tag(&self, id: &str) -> &[Arc<CommandFunction>] {
        self.tags.get(id).map_or(&[], Vec::as_slice)
    }

    pub fn function_names(&self) -> impl Iterator<Item = &str> {
        self.functions.iter().map(|(id, _)| id)
    }

    pub fn available_tags(&self) -> impl Iterator<Item = &str> {
        self.tags.iter().map(|(id, _)| id)
    }

    pub fn reload(resource_manager: &MultiPackResourceManager, function_compilation_permission: i32) -> ServerFunctionLibrary {
        let tags_loader = TagLoader::new(TAG_DIRECTORY);
        let mut functions_to_load: Vec<_> = resource_manager.list_resources(FUNCTION_DIRECTORY, FUNCTION_EXTENSION).into_iter().collect();
        identifier_order(&mut functions_to_load);
        let mut result: JavaHashMap<usize> = JavaHashMap::new(identifier_hash);
        let mut sources = Vec::with_capacity(functions_to_load.len());
        for (resource_id, resource) in functions_to_load {
            let (namespace, path) = resource_id.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[FUNCTION_DIRECTORY.len() + 1..path.len() - FUNCTION_EXTENSION.len()]);
            result.insert(id.clone(), sources.len());
            sources.push((id, resource));
        }
        let compilation_context = create_compilation_context(function_compilation_permission);
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(sources.len()).max(1);
        let next = AtomicUsize::new(0);
        let mut compiled: Vec<Option<Result<CommandFunction, String>>> = (0..sources.len()).map(|_| None).collect();
        let tags = std::thread::scope(|scope| {
            let tags = std::thread::Builder::new().name(worker_name()).spawn_scoped(scope, || tags_loader.load(resource_manager)).expect("worker thread");
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    let (sources, next, compilation_context) = (&sources, &next, &compilation_context);
                    std::thread::Builder::new()
                        .name(worker_name())
                        .spawn_scoped(scope, move || {
                            let mut done = Vec::new();
                            loop {
                                let index = next.fetch_add(1, Ordering::Relaxed);
                                let Some((id, resource)) = sources.get(index) else { break };
                                let function = resource.supplier.read().map_err(|e| format!("java.io.IOException: {e}")).and_then(|bytes| {
                                    CommandFunction::from_lines(id, compilation_context, &read_lines(&bytes))
                                        .map_err(|e| format!("java.lang.IllegalArgumentException: {e}"))
                                });
                                done.push((index, function));
                            }
                            done
                        })
                        .expect("worker thread")
                })
                .collect();
            for handle in handles {
                for (index, function) in handle.join().expect("function compilation") {
                    compiled[index] = Some(function);
                }
            }
            tags.join().expect("tag loading")
        });
        let mut functions: JavaHashMap<Arc<CommandFunction>> = JavaHashMap::new(identifier_hash);
        for (id, &index) in result.iter() {
            match compiled[index].take().unwrap() {
                Ok(function) => {
                    functions.insert(id.to_string(), Arc::new(function));
                }
                Err(exception) => error!("Failed to load function {id}\njava.util.concurrent.CompletionException: {exception}\nCaused by: {exception}"),
            }
        }
        let tags = tags_loader.build(&tags, &|id, _| functions.get(id).cloned(), &Arc::ptr_eq);
        ServerFunctionLibrary { functions, tags }
    }
}
