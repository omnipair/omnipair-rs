#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../../..");
const idlPath = path.join(repoRoot, "packages/program-interface/src/idl_v2.json");
const root = path.join(scriptDir, "../src/v2");
const typesDir = path.join(root, "types");
const instructionsDir = path.join(root, "instructions");
const accountsDir = path.join(root, "accounts");
const generatedHeader =
  "// This V2 decoder code is generated from packages/program-interface/src/idl_v2.json.\n";

const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));

function snake(name) {
  return name
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .replace(/__/g, "_")
    .toLowerCase();
}

function pascal(name) {
  return name
    .split("_")
    .map((part) => (part ? part[0].toUpperCase() + part.slice(1) : ""))
    .join("");
}

function discriminatorHex(discriminator) {
  return `0x${Buffer.from(discriminator ?? []).toString("hex")}`;
}

function rustType(type) {
  if (typeof type === "string") {
    return type === "pubkey" ? "solana_pubkey::Pubkey" : type;
  }
  if (type.defined) return type.defined.name;
  if (type.option) return `Option<${rustType(type.option)}>`;
  if (type.array) return `[${rustType(type.array[0])}; ${type.array[1]}]`;
  if (type.vec) return `Vec<${rustType(type.vec)}>`;
  throw new Error(`unsupported IDL type: ${JSON.stringify(type)}`);
}

function referencesDefinedType(type) {
  if (!type || typeof type === "string") return false;
  if (type.defined) return true;
  if (type.option) return referencesDefinedType(type.option);
  if (type.array) return referencesDefinedType(type.array[0]);
  if (type.vec) return referencesDefinedType(type.vec);
  return false;
}

function fieldLines(fields = [], indent = "    ") {
  return fields
    .map((field) => `${indent}pub ${field.name}: ${rustType(field.type)},`)
    .join("\n");
}

function deriveLine() {
  return "#[derive(CarbonDeserialize, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]";
}

function writeFile(filePath, contents) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${contents.replace(/\n{3,}/g, "\n\n").trimEnd()}\n`);
}

function writeTypes() {
  const typeNames = [];

  for (const typeDef of idl.types ?? []) {
    const name = typeDef.name;
    const mod = snake(name);
    const type = typeDef.type;
    typeNames.push({ name, mod });

    let body;
    if (type.kind === "struct") {
      const needsSuper = (type.fields ?? []).some((field) => referencesDefinedType(field.type));
      body = `${generatedHeader}${needsSuper ? "use super::*;\n\n" : ""}use carbon_core::{borsh, CarbonDeserialize};\n\n${deriveLine()}\npub struct ${name} {\n${fieldLines(type.fields)}\n}\n`;
    } else if (type.kind === "enum") {
      const variants = (type.variants ?? [])
        .map((variant) => {
          if (!variant.fields) return `    ${variant.name},`;
          if (Array.isArray(variant.fields)) {
            const values = variant.fields.map((field) => rustType(field.type ?? field)).join(", ");
            return `    ${variant.name}(${values}),`;
          }
          return `    ${variant.name} {\n${fieldLines(variant.fields, "        ")}\n    },`;
        })
        .join("\n");
      const needsSuper = (type.variants ?? []).some((variant) =>
        (variant.fields ?? []).some((field) => referencesDefinedType(field.type ?? field))
      );
      body = `${generatedHeader}${needsSuper ? "use super::*;\n\n" : ""}use carbon_core::{borsh, CarbonDeserialize};\n\n${deriveLine()}\npub enum ${name} {\n${variants}\n}\n`;
    } else {
      throw new Error(`unsupported IDL type kind: ${type.kind}`);
    }

    writeFile(path.join(typesDir, `${mod}.rs`), body);
  }

  writeFile(
    path.join(typesDir, "mod.rs"),
    `${generatedHeader}${typeNames.map(({ mod }) => `pub mod ${mod};`).join("\n")}\n${typeNames
      .map(({ name, mod }) => `pub use ${mod}::${name};`)
      .join("\n")}\n`
  );
}

function accountType(name) {
  const typeDef = (idl.types ?? []).find((candidate) => candidate.name === name);
  if (!typeDef || typeDef.type.kind !== "struct") {
    throw new Error(`missing struct type for account ${name}`);
  }
  return typeDef;
}

function writeAccounts() {
  const accounts = [];

  for (const account of idl.accounts ?? []) {
    const name = account.name;
    const mod = snake(name);
    const typeDef = accountType(name);
    const needsTypes = (typeDef.type.fields ?? []).some((field) =>
      referencesDefinedType(field.type)
    );
    accounts.push({ name, mod });

    writeFile(
      path.join(accountsDir, `${mod}.rs`),
      `${generatedHeader}${needsTypes ? "use super::super::types::*;\n\n" : ""}use carbon_core::{borsh, CarbonDeserialize};\n\n${deriveLine()}\n#[carbon(discriminator = "${discriminatorHex(account.discriminator)}")]\npub struct ${name} {\n${fieldLines(typeDef.type.fields)}\n}\n`
    );
  }

  const enumVariants = accounts
    .map(({ name, mod }) => `    ${name}(${mod}::${name}),`)
    .join("\n");
  const decodeBranches = accounts
    .map(
      ({ name, mod }) => `        if let Some(decoded_account) = ${mod}::${name}::deserialize(account.data.as_slice()) {\n            return Some(carbon_core::account::DecodedAccount {\n                lamports: account.lamports,\n                data: OmnipairV2Account::${name}(decoded_account),\n                owner: account.owner,\n                executable: account.executable,\n                rent_epoch: account.rent_epoch,\n            });\n        }`
    )
    .join("\n\n");

  writeFile(
    path.join(accountsDir, "mod.rs"),
    `${generatedHeader}use carbon_core::account::AccountDecoder;\nuse carbon_core::deserialize::CarbonDeserialize;\n\nuse super::OmnipairV2Decoder;\n\n${accounts
      .map(({ mod }) => `pub mod ${mod};`)
      .join("\n")}\n\n#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Hash)]\npub enum OmnipairV2Account {\n${enumVariants}\n}\n\nimpl<'a> AccountDecoder<'a> for OmnipairV2Decoder {\n    type AccountType = OmnipairV2Account;\n\n    fn decode_account(\n        &self,\n        account: &solana_account::Account,\n    ) -> Option<carbon_core::account::DecodedAccount<Self::AccountType>> {\n${decodeBranches}\n\n        None\n    }\n}\n`
  );
}

function eventAsInstruction(event) {
  const typeDef = (idl.types ?? []).find((candidate) => candidate.name === event.name);
  if (!typeDef || typeDef.type.kind !== "struct") {
    throw new Error(`missing struct type for event ${event.name}`);
  }
  return {
    name: event.name,
    discriminator: event.discriminator,
    accounts: [],
    args: typeDef.type.fields ?? [],
  };
}

function writeInstructionFile(instruction, name, mod) {
  const args = instruction.args ?? [];
  const accounts = instruction.accounts ?? [];
  const argsFields = args.map((arg) => `    pub ${arg.name}: ${rustType(arg.type)},`).join("\n");
  const accountStructName = `${name}InstructionAccounts`;
  const accountFields = accounts
    .map((account) => `    pub ${account.name}: solana_pubkey::Pubkey,`)
    .join("\n");
  const accountReads = accounts
    .map((account) => `        let ${account.name} = next_account(&mut iter)?;`)
    .join("\n");
  const accountInits = accounts.map((account) => `            ${account.name},`).join("\n");
  const arrangeImpl = accounts.length
    ? `\n#[derive(Debug, PartialEq, Eq, Clone, Hash, serde::Serialize, serde::Deserialize)]\npub struct ${accountStructName} {\n${accountFields}\n}\n\nimpl carbon_core::deserialize::ArrangeAccounts for ${name} {\n    type ArrangedAccounts = ${accountStructName};\n\n    fn arrange_accounts(accounts: &[solana_instruction::AccountMeta]) -> Option<Self::ArrangedAccounts> {\n        let mut iter = accounts.iter();\n${accountReads}\n\n        Some(${accountStructName} {\n${accountInits}\n        })\n    }\n}\n`
    : "";
  const imports = accounts.length
    ? "use carbon_core::{account_utils::next_account, borsh, CarbonDeserialize};"
    : "use carbon_core::{borsh, CarbonDeserialize};";

  writeFile(
    path.join(instructionsDir, `${mod}.rs`),
    `${generatedHeader}use super::super::types::*;\n\n${imports}\n\n${deriveLine()}\n#[carbon(discriminator = "${discriminatorHex(instruction.discriminator)}")]\npub struct ${name} {\n${argsFields}\n}\n${arrangeImpl}`
  );
}

function writeInstructions() {
  const instructions = [
    ...(idl.instructions ?? []).map((instruction) => ({
      ...instruction,
      name: pascal(instruction.name),
    })),
    ...(idl.events ?? []).map(eventAsInstruction),
  ];
  const names = instructions.map((instruction) => ({
    name: instruction.name,
    mod: snake(instruction.name),
  }));

  for (const instruction of instructions) {
    writeInstructionFile(instruction, instruction.name, snake(instruction.name));
  }

  const enumVariants = names
    .map(({ name, mod }) => `    ${name}(${mod}::${name}),`)
    .join("\n");
  const decodeBranches = names
    .map(
      ({ name, mod }) => `        if let Some(decoded) = ${mod}::${name}::deserialize(instruction.data.as_slice()) {\n            return Some(carbon_core::instruction::DecodedInstruction {\n                program_id: instruction.program_id,\n                accounts: instruction.accounts.clone(),\n                data: OmnipairV2Instruction::${name}(decoded),\n            });\n        }`
    )
    .join("\n\n");

  writeFile(
    path.join(instructionsDir, "mod.rs"),
    `${generatedHeader}use carbon_core::deserialize::CarbonDeserialize;\n\nuse super::{OmnipairV2Decoder, PROGRAM_ID};\n\n${names
      .map(({ mod }) => `pub mod ${mod};`)
      .join("\n")}\n\n#[derive(carbon_core::InstructionType, serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug, Clone, Hash)]\npub enum OmnipairV2Instruction {\n${enumVariants}\n}\n\nimpl<'a> carbon_core::instruction::InstructionDecoder<'a> for OmnipairV2Decoder {\n    type InstructionType = OmnipairV2Instruction;\n\n    fn decode_instruction(\n        &self,\n        instruction: &solana_instruction::Instruction,\n    ) -> Option<carbon_core::instruction::DecodedInstruction<Self::InstructionType>> {\n        if instruction.program_id != PROGRAM_ID {\n            return None;\n        }\n\n${decodeBranches}\n\n        None\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    use carbon_core::deserialize::ArrangeAccounts;\n    use carbon_core::instruction::InstructionDecoder;\n    use solana_instruction::{AccountMeta, Instruction};\n    use solana_pubkey::Pubkey;\n\n    #[test]\n    fn decodes_and_arranges_v2_swap_instruction() {\n        let mut data = vec![0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];\n        data.push(0);\n        data.extend_from_slice(&123_u64.to_le_bytes());\n        data.extend_from_slice(&45_u64.to_le_bytes());\n        let accounts = (0..13)\n            .map(|_| AccountMeta::new(Pubkey::new_unique(), false))\n            .collect::<Vec<_>>();\n        let instruction = Instruction {\n            program_id: PROGRAM_ID,\n            accounts: accounts.clone(),\n            data,\n        };\n\n        let decoded = OmnipairV2Decoder\n            .decode_instruction(&instruction)\n            .expect(\"swap should decode\");\n\n        match decoded.data {\n            OmnipairV2Instruction::Swap(swap) => {\n                assert!(matches!(\n                    swap.args.asset_in,\n                    crate::v2::types::MarketAsset::Base\n                ));\n                assert_eq!(swap.args.exact_asset_in, 123);\n                assert_eq!(swap.args.min_asset_out, 45);\n            }\n            other => panic!(\"unexpected instruction: {other:?}\"),\n        }\n\n        let arranged = swap::Swap::arrange_accounts(&accounts).expect(\"swap accounts arrange\");\n        assert_eq!(arranged.market, accounts[0].pubkey);\n        assert_eq!(arranged.program, accounts[12].pubkey);\n    }\n}\n`
  );
}

function writeRoot() {
  writeFile(
    path.join(root, "mod.rs"),
    `${generatedHeader}pub const PROGRAM_ID: solana_pubkey::Pubkey =\n    solana_pubkey::Pubkey::from_str_const(\"oMNi2XGwWxDbEvhS2pWRQ6dtw8GkNBV42hfLZD6WmMF\");\n\npub struct OmnipairV2Decoder;\n\npub mod accounts;\npub mod instructions;\npub mod types;\n`
  );
}

function rustfmtGeneratedFiles() {
  const files = [];
  const visit = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) visit(entryPath);
      if (entry.isFile() && entry.name.endsWith(".rs")) files.push(entryPath);
    }
  };
  visit(root);

  const result = spawnSync("rustfmt", files, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error("rustfmt failed for generated V2 decoder files");
  }
}

fs.rmSync(root, { recursive: true, force: true });
for (const dir of [typesDir, instructionsDir, accountsDir]) {
  fs.mkdirSync(dir, { recursive: true });
}

writeTypes();
writeAccounts();
writeInstructions();
writeRoot();
rustfmtGeneratedFiles();

console.log(`Generated V2 decoder from ${path.relative(repoRoot, idlPath)}`);
