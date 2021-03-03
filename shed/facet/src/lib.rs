/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![deny(warnings, missing_docs, clippy::all, broken_intra_doc_links)]

//! # Overview
//!
//! `facet` provides a way for objects to contain factory-created trait
//! objects, separating the construction of those trait objects out to a
//! factory, and providing compile-time checked dependency injection.
//!
//! There are three parts: "Facets", "Factories", and "Containers".
//!
//! ## Facet
//!
//! A **facet** is defined by a trait, which will be used as an interface to
//! an object that the containers will store.  Any trait that can be turned
//! into a trait object can be a facet trait.  To make a trait into a facet,
//! add the `#[facet::facet]` attribute:
//!
//! ```
//! #[facet::facet]
//! trait MyTrait {
//!     fn do_something(&self);
//! }
//! ```
//!
//! Marking a trait as a facet generates several additional types:
//!
//! ### Ref Trait
//!
//! The reference trait (`MyTraitRef`) will be implemented by containers
//! that support this trait.
//!
//! It provides a single method, named by the snake-case equivalent of
//! the facet trait name, that returns a reference to the trait implementation
//! the container holds.
//!
//! The reference trait can be used as a trait bound in other
//! parts of the program, and is the preferred way to hand around
//! references to an implementation of the facet trait.  For example:
//!
//! ```
//! # #[facet::facet] trait MyTrait { fn do_something(&self); }
//! fn my_function(container: impl MyTraitRef) {
//!     container.my_trait().do_something();
//! }
//! ```
//!
//! ### Arc Trait
//!
//! The arc trait (`MyTraitArc`) will similarly be implemented by containers
//! that support the trait, however it provides access to a cloneable `Arc`
//! of the trait implementation, for when detaching a handle to the
//! implementation is necessary.
//!
//! ```
//! # #[facet::facet] trait MyTrait {}
//! fn send(_my_trait: ArcMyTrait) {
//!     // ...
//! }
//!
//! fn my_function(container: impl MyTraitArc) {
//!     let arc_impl = container.my_trait_arc();
//!     send(arc_impl);
//! }
//! ```
//!
//! ### Arc Alias
//!
//! The arc alias (`ArcMyTrait`) is an alias to `Arc<dyn MyTrait + Send + Sync>`
//! that is used in factory definitions (see below).
//!
//! ## Factory
//!
//! A **factory** is defined by implementing a set of methods on a struct,
//! and annotating that `impl` block with `#[facet::factory]`.
//!
//! Each method in the factory defines how one facet can be built.  The
//! factory can also take parameters, which are also available to these
//! factory methods.
//!
//! The return type of each factory method should be an `Arc`-wrapped facet
//! (e.g.  `Arc<dyn Facet + Send + Sync>`) or a `Result` containing an
//! `Arc`-wrapped facet as its `Ok` variant, and an error that implements
//! `std::error:Error` as its `Err` variant.
//!
//! The factory methods must all take `&self`, and the additional parameters
//! must all be either:
//!
//! * a factory parameter by reference, where the name must match that
//!   given in the factory attribute, and the type is a borrowed version
//!   of the parameter type; or
//!
//! * another facet that this factory can build, where the name must match the
//!   name of the method that builds the facet, and the type must be a reference
//!   to an `Arc`-wrapped facet.
//!
//! You can use the arc alias generated by the facet macro (`ArcMyTrait`) as a
//! convenience for specifying the `Arc`-wrapped facets in both parameters and
//! return types.
//!
//! The dependencies between facets are defined by which factory methods depend
//! on which other factory methods.  When factory methods depend on each other
//! they must not form cycles, or you will get an error at compile time.
//!
//! The factory is free to select any implementor of a facet trait as the
//! implementation it returns.  It should wrap this as a facet in an `Arc`
//! using `Arc::new(...)`.
//!
//! ```
//! # #[facet::facet] trait MyTrait {}
//! # #[facet::facet] trait OtherTrait {}
//! # use anyhow::Error;
//! # use std::sync::Arc;
//! # struct MyTraitImpl { config: () }
//! # impl MyTrait for MyTraitImpl {}
//! # impl MyTraitImpl {
//! #     fn new(_name: &str, _config: &str) -> Result<MyTraitImpl, Error> {
//! #         Ok(MyTraitImpl { config: () })
//! #     }
//! # }
//! # struct OtherTraitImpl;
//! # impl OtherTrait for OtherTraitImpl {}
//! # impl OtherTraitImpl {
//! #     fn new(_my_trait: ArcMyTrait, value: u32) -> OtherTraitImpl {
//! #         OtherTraitImpl
//! #     }
//! # }
//! struct MyFactory {
//!     config: String,
//! }
//!
//! #[facet::factory(name: String, value: u32)]
//! impl MyFactory {
//!     fn my_trait(&self, name: &str) -> Result<ArcMyTrait, Error> {
//!         Ok(Arc::new(
//!             MyTraitImpl::new(name, self.config.as_str())?
//!         ))
//!     }
//!
//!     fn other_trait(&self, my_trait: &ArcMyTrait, value: &u32) -> ArcOtherTrait {
//!         Arc::new(OtherTraitImpl::new(my_trait.clone(), *value))
//!     }
//! }
//! ```
//!
//! Multiple factories can be defined and each one can have different
//! parameters, different dependency relationships between facets, and select
//! different facet implementations.  You could have a production factory that
//! generates facets with production implementations, and a test factory that
//! generates test doubles.  The factory implementation can also select which
//! facet implementation it should use at run-time, for example, based on
//! configuration stored in the factory or the parameters to the factory.
//!
//! The macro will define a `build` method for each factory, which can be used
//! to build containers (see below).
//!
//! ## Containers
//!
//! A **container** is a struct that contains facets.  Each field of a
//! container can either be a normal field, for which you must provide an
//! initializer, or a facet, which must have a facet trait as its type
//! and whose name must match that of the facet.
//!
//! Initializers for normal fields may reference any of the facets that
//! are part of the container.
//!
//! For example:
//!
//! ```
//! # #[facet::facet] trait OtherTrait { fn get_name(&self) -> &str; }
//! #[facet::container]
//! struct MyContainer {
//!     #[init(other_trait.get_name().to_string())]
//!     name: String,
//!
//!     #[facet]
//!     other_trait: dyn OtherTrait,
//! }
//! ```
//!
//! Containers can be contructed using the `build` method of a factory.
//! The build method must be passed the parameters defined on the factory
//! attribute and these will be used as inputs for building this container.
//!
//! ```
//! # struct MyFactory { config: String }
//! # #[facet::factory(name: String, value: u32)] impl MyFactory {}
//! # #[facet::container] struct MyContainer {}
//! # fn main() -> Result<(), anyhow::Error> {
//! let factory = MyFactory { config: String::from("config") };
//! let name = String::from("name");
//! let my_container = factory.build::<MyContainer>(name, 42)?;
//! #     Ok(())
//! # }
//! ```
//!
//! The `build` method always returns `Result<Container, FactoryError>`, even
//! if none of the factory methods are fallible.  If no methods are fallible
//! then the result will always be `Ok`.
//!
//! ## Async
//!
//! Async facets can be supported by using the `async-trait` crate.
//!
//! Async factory methods are supported.  To make a factory async, mark one or
//! more methods as `async`:
//! ```
//! # #[facet::facet] trait MyTrait {}
//! #[facet::facet]
//! #[async_trait::async_trait]
//! trait OtherTrait {
//!     async fn do_something(&self);
//! }
//!
//! # use anyhow::Error;
//! # use std::sync::Arc;
//! # struct MyTraitImpl { config: () }
//! # impl MyTrait for MyTraitImpl {}
//! # impl MyTraitImpl {
//! #     async fn new(_name: &str, _config: &str) -> Result<MyTraitImpl, Error> {
//! #         Ok(MyTraitImpl { config: () })
//! #     }
//! # }
//! # struct OtherTraitImpl;
//! # #[async_trait::async_trait]
//! # impl OtherTrait for OtherTraitImpl {
//! #     async fn do_something(&self) {}
//! # }
//! # impl OtherTraitImpl {
//! #     fn new(_my_trait: ArcMyTrait, value: u32) -> OtherTraitImpl {
//! #         OtherTraitImpl
//! #     }
//! # }
//! # struct MyAsyncFactory { config: String }
//! #[facet::factory(name: String, value: u32)]
//! impl MyAsyncFactory {
//!     async fn my_trait(&self, name: &str) -> Result<ArcMyTrait, Error> {
//!         Ok(Arc::new(
//!             MyTraitImpl::new(name, self.config.as_str()).await?
//!         ))
//!     }
//!
//!     fn other_trait(&self, my_trait: &ArcMyTrait, value: &u32) -> ArcOtherTrait {
//!         Arc::new(OtherTraitImpl::new(my_trait.clone(), *value))
//!     }
//! }
//! ```
//!
//! For async factories, the build method is async:
//!
//! ```
//! # #[facet::facet] trait MyTrait {}
//! # struct MyTraitImpl;
//! # impl MyTrait for MyTraitImpl {}
//! # struct MyAsyncFactory { config: String }
//! # #[facet::factory(name: String, value: u32)]
//! # impl MyAsyncFactory {
//! #     async fn my_trait(&self) -> ArcMyTrait {
//! #        std::sync::Arc::new(MyTraitImpl)
//! #     }
//! # }
//! # #[facet::container] struct MyContainer {}
//! # #[tokio::main]
//! # async fn main() -> Result<(), anyhow::Error> {
//! let factory = MyAsyncFactory { config: String::from("config") };
//! let name = String::from("name");
//! let my_container = factory.build::<MyContainer>(name, 42).await?;
//! #     Ok(())
//! # }
//! ```
//!
//! The build method will attempt to build facets concurrently where it can.

extern crate facet_proc_macros;
pub use facet_proc_macros::{container, facet, factory};

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[doc(hidden)]
pub extern crate async_trait;

#[doc(hidden)]
pub extern crate futures;

/// An error during construction by a facet factory.
#[derive(Debug, Error)]
pub enum FactoryError {
    /// A facet failed to build.
    #[error("failed to build '{name}'")]
    FacetBuildFailed {
        /// The name of the facet that failed to build.
        name: &'static str,

        /// The error encountered when building the facet.
        source: anyhow::Error,
    },
}

// Clonable wrapper for `FactoryError` in async builders.
#[doc(hidden)]
#[derive(Clone)]
pub struct AsyncFactoryError {
    error: Arc<Mutex<Option<FactoryError>>>,
}

impl From<FactoryError> for AsyncFactoryError {
    fn from(err: FactoryError) -> AsyncFactoryError {
        AsyncFactoryError {
            error: Arc::new(Mutex::new(Some(err))),
        }
    }
}

impl AsyncFactoryError {
    #[doc(hidden)]
    pub fn factory_error(self) -> FactoryError {
        let mut error = self.error.lock().unwrap();
        // This method should be called once at the first point that
        // `build_needed` fails, and so it is invalid for the error to already
        // have been taken.
        error
            .take()
            .expect("bug in #[facet::factory]: factory error already taken")
    }
}

// Trait implemented by containers that are buildable by factory builders.
#[doc(hidden)]
pub trait Buildable<B>: Sized {
    fn build(builder: B) -> Result<Self, FactoryError>;
}

// Trait implemented by containers that are buildable by async factory builders.
// Desugared async-trait so that the builder lifetime can be specified.
#[doc(hidden)]
pub trait AsyncBuildable<'builder, B>: Sized {
    fn build_async(
        builder: B,
    ) -> Pin<Box<dyn Future<Output = Result<Self, FactoryError>> + Send + 'builder>>;
}

// Trait implemented by factory builders that can build facets of type T.
#[doc(hidden)]
pub trait Builder<T: Sized> {
    fn build(&mut self) -> Result<T, FactoryError>;
}

// Trait implemented by factory builders that can asynchronously build facets
// of type T.
#[doc(hidden)]
pub trait AsyncBuilderFor<T: Sized> {
    // Mark this facet type (and its dependencies) as needed.
    fn need(&mut self);

    // Get the built instance of this facet.
    fn get(&self) -> T;
}

// Trait implemented by factory builds to trigger parallel async build of
// facets marked as needed.
#[doc(hidden)]
#[async_trait::async_trait]
pub trait AsyncBuilder {
    async fn build_needed(&mut self) -> Result<(), FactoryError>;
}

// Trait implemented by containers that can provide a reference to facets of
// type T.
#[doc(hidden)]
pub trait FacetRef<T: ?Sized + Send + Sync + 'static> {
    fn facet_ref(&self) -> &T;
}

// Trait implemented by containers that can provide an arc to facets of
// type T.
#[doc(hidden)]
pub trait FacetArc<T: ?Sized + Send + Sync + 'static> {
    fn facet_arc(&self) -> Arc<T>;
}
