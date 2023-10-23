// Copyright © 2023 Luke Chambers
// This file is part of Backtrack.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License. You may obtain a copy of
// the License at <http://www.apache.org/licenses/LICENSE-2.0>.
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
// License for the specific language governing permissions and limitations under
// the License.

#import "/src/lib.typ"
#import lib: versions

#let version = lib.current-version

#let cmp-to(other) = [
  This is
  #if version.cmpable < other.cmpable [
    less than
  ] else if version.cmpable == other.cmpable [
    equal to
  ] else [
    greater than
  ]
  Typst #other.displayable.
]

#let body = [
  You are using Typst v#version.displayable!

  #cmp-to(versions.v0-3-0)
  #cmp-to(versions.v0-8-0)
  #cmp-to(versions.v2023-01-30)
  #cmp-to(versions.v2023-02-25)
  #cmp-to(versions.post-v0-8-0(0, 9, 0))
  #cmp-to(versions.post-v0-8-0((1, 2), 3))

  The minor version is
  #if calc.even(version.observable.at(1)) [
    even.
  ] else [
    odd.
  ]
]

#if version.cmpable >= versions.v2023-03-21.cmpable {
  set text(font: "Libertinus Serif")
  show raw: set text(font: "Libertinus Mono")
  show math.equation: set text(font: "Libertinus Math")
  body
} else {
  set text(family: "Libertinus Serif")
  show raw: set text(family: "Libertinus Mono")
  show math.formula: set text(family: "Libertinus Math")
  body
}
