# Implementation Plan

Overarching features I want:
- I want a fully features knowledge management CLI that is able to initialize a new knowledge base in a repo.
- This knowledge base (or gnosis) should be made up of individual "packages" which is basically the folder directly under the "gnosis" folder in a given repo, where each folder represents a unit of knowledge which should be able to be accessed and updates by a agent
- The gnosis folder should follow the Open Knowledge Format standard, and as much as possible of the actual structuring etc. should be handled by other tools such as git
- I want to be able to init a new gnosis, I want to be able to init a new package within this gnosis, I want to be able to define a standard (which should be dynamic, i.e. the user should be able to change the standard format easily)
- I want to be able to have a .lock file of the gnosis, with dependencies etc. so one package can be dependant on another to make referencing easier
- I should be able to add a gnosis repo, so for example a link to a public or private beno-gnosis github repo with the correct package.yml / package.json file should be able to be resolved, and each modular unit within this repo should be able to be downloaded seperately

Technology
- I want to use the Open Knowledge Format for storing of knowledge
- I want basic agent skills/ to enable LLM agents to update the knowledge base itself, which should be the driver of this plan
- I want a Go Lang CLI that makes a user able to manage their gnosis structure, but it should have nothing to do with the content of the gnosis, that is the agents job to update
- I dont really understand "CODEOWNERS" github standard, but it might be something that is useful for enforcing a given team, role or user to be able to approve changes to the package once a persons agent has updated it with new relevant knowledge. I want one owner per package, and if several owners make more sense, it should be several packages

Overarching:
I want you to thoroughly research the best practise for each specific area and practise, and if there is industry standard tooling for a similar/same job, this should be preferred over making "in house" solutions for everything.

Example schema for YAML (package.yml):

gnosis_version: 1

package: beno-migrering
version: 1.0.0

owner:
    team: beno-platform

dependencies:
    - beno-core
    - beno-snowflake

Example registry for the packages: (input in the fields are irrelevant, but the description could double as the description used for the index.md in the OKF format)
beno-migrering:
    description: Used for migrating SQL solutions from one platform to another
    owner: beno-platform
    source:
      type: git
      repository: git@github.internal:beno/wiki-packages.git
      path: migrering
      default_branch: main

Core principle
The registry owns package discovery.
The source repository owns package content and approval.
Consumers should not need to know where a package physically lives.
gnosis pull sb1u-risk-mislighold
gnosis pull beno-migrering

Suggested syntax:

gnosis add sb1u-risk-mislighold | If the correct path is installed, a user should just be able to make a add and then name, or just type gnosis add and get a list of all discovered packages

gnosis init | should init a new gnosis directory that is controlled by the CLI and agents are allowed to act on. 

gnosis pull / propose | Should pull new updates from a given directory. Should use git backend, so that if there are merge conflics, everything can be handled exactly the same as with git. So gnosis rebase -X ours origin/main should work, gnosis merge should work etc. SO here it is important to enforce a standard to be able to use these commands. If this is bad design, please correct me. Propose command should be similar to push, but as we allow only a single source of truth in a given package, it should be a proposed change (pull request) that the business owner must approve themselves

# Skills
I want you to also make a suite of skills that are able to be used by agents to interact with the gnosis knowledge base.

These skills should include:
- gnosis-navigate | How an agent should navigate the OKF structure and actually absort information
- gnosis-update | How an agent should update a given knowledge base
- gnosis-cli | How an agent should interact with the gnosis cli to simplify suggestions for change etc.

I am also open to add more relevant skills. These skills should only be "meta skills", i.e. nothing to do about specific business logic outside the gnosis system itself

# Setup
I want this system to be super flexible and dynamic. For example, I want each package to define a granularity themselves, where they adhere to a given "schema", or structure for how their markdown files within the OKF are handles. I want the gnosis tool to handle the metadata and index generation in some way, but this should be defined in a file within the package itself, not in the gnosis cli.

Ask me as many questions you need before you start implementing this tool. I want you to be 100% sure about my needs, intentions, and when you have done this, I want everything you learned from my questions to be added to its own gnosis-architechture package which can be referenced later by other models that need to know what the overarching goal and decisions that were made in implementing this CLI and structure.


