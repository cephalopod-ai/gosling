#!/usr/bin/env ruby

require "yaml"

REPOSITORY_ROOT = File.expand_path("../..", __dir__)
WORKFLOW_DIRECTORY = ENV.fetch("GOSLING_WORKFLOW_DIRECTORY", File.join(REPOSITORY_ROOT, ".github", "workflows"))

EXPECTED_WORKFLOW_PERMISSIONS = {
  "release.yml" => {"contents" => "read"},
  "build-cli.yml" => {"contents" => "read"},
  "bundle-desktop-linux.yml" => {"contents" => "read"},
  "bundle-desktop-windows.yml" => {"contents" => "read"},
  "publish-npm.yml" => {"contents" => "read"},
  "canary.yml" => {"contents" => "read"},
  "close-release-pr-on-tag.yaml" => {"contents" => "read"}
}.freeze

EXPECTED_JOB_PERMISSIONS = {
  "release.yml" => {
    "build-cli" => {"contents" => "read"},
    "install-script" => {"contents" => "read"},
    "bundle-desktop" => {"contents" => "read"},
    "bundle-desktop-intel" => {"contents" => "read"},
    "bundle-desktop-linux" => {"contents" => "read"},
    "bundle-desktop-windows" => {"contents" => "read", "id-token" => "write"},
    "release" => {"attestations" => "write", "contents" => "write", "id-token" => "write"}
  },
  "publish-npm.yml" => {
    "build-cli" => {"contents" => "read"},
    "build" => {"contents" => "read"},
    "publish" => {"contents" => "read", "id-token" => "write"}
  },
  "bundle-desktop-windows.yml" => {
    "build-desktop-windows" => {"contents" => "read"},
    "sign-desktop-windows" => {"id-token" => "write"},
    "package-desktop-windows" => {"contents" => "read"}
  },
  "canary.yml" => {
    "prepare-version" => {"contents" => "read"},
    "build-cli" => {"contents" => "read"},
    "install-script" => {"contents" => "read"},
    "bundle-desktop" => {"contents" => "read"},
    "bundle-desktop-intel" => {"contents" => "read"},
    "bundle-desktop-linux" => {"contents" => "read"},
    "bundle-desktop-windows" => {"contents" => "read"},
    "release" => {"attestations" => "write", "contents" => "write", "id-token" => "write"}
  },
  "close-release-pr-on-tag.yaml" => {
    "close-release-pr" => {"pull-requests" => "write"},
    "trigger-patch-release" => {"actions" => "write"}
  }
}.freeze

def fail_contract(message)
  warn "Phase 2 permission check failed: #{message}"
  exit 1
end

def load_workflow(file_name)
  path = File.join(WORKFLOW_DIRECTORY, file_name)
  YAML.load_file(path)
rescue Psych::SyntaxError => error
  fail_contract("#{file_name} is not valid YAML: #{error.message}")
end

def normalized_permissions(value)
  fail_contract("permissions must be a mapping, found #{value.inspect}") unless value.is_a?(Hash)

  value.transform_keys(&:to_s).sort.to_h
end

EXPECTED_WORKFLOW_PERMISSIONS.each do |file_name, expected_permissions|
  workflow = load_workflow(file_name)
  actual_permissions = normalized_permissions(workflow["permissions"])
  expected_permissions = expected_permissions.sort.to_h

  unless actual_permissions == expected_permissions
    fail_contract("#{file_name} workflow permissions are #{actual_permissions.inspect}; expected #{expected_permissions.inspect}")
  end

  if actual_permissions.value?("write")
    fail_contract("#{file_name} grants a workflow-wide write permission")
  end

  expected_jobs = EXPECTED_JOB_PERMISSIONS.fetch(file_name, {})
  expected_jobs.each do |job_name, expected_job_permissions|
    job = workflow.fetch("jobs").fetch(job_name)
    actual_job_permissions = normalized_permissions(job["permissions"])
    expected_job_permissions = expected_job_permissions.sort.to_h

    unless actual_job_permissions == expected_job_permissions
      fail_contract("#{file_name} job #{job_name} permissions are #{actual_job_permissions.inspect}; expected #{expected_job_permissions.inspect}")
    end
  end

  workflow.fetch("jobs").each do |job_name, job|
    next unless job.key?("permissions")

    normalized_permissions(job["permissions"]).each do |permission, access|
      next unless access == "write"

      allowed = expected_jobs.fetch(job_name, {})[permission] == "write"
      fail_contract("#{file_name} job #{job_name} has unapproved #{permission}: write") unless allowed
    end
  end
end

issueops = load_workflow("pr-comment-build-cli.yml")
issueops_build_permissions = normalized_permissions(issueops.fetch("jobs").fetch("build-cli").fetch("permissions"))
unless issueops_build_permissions == {"contents" => "read"}
  fail_contract("pr-comment-build-cli.yml must pass only contents: read to build-cli.yml")
end

close_release = load_workflow("close-release-pr-on-tag.yaml")
close_job = close_release.fetch("jobs").fetch("close-release-pr")
dispatch_job = close_release.fetch("jobs").fetch("trigger-patch-release")

if close_job.fetch("steps").any? { |step| step["uses"]&.start_with?("actions/checkout@") }
  fail_contract("close-release-pr-on-tag.yaml must not checkout repository code in the PR-write job")
end

unless close_job.dig("outputs", "branch") == "${{ steps.version.outputs.branch }}"
  fail_contract("close-release-pr-on-tag.yaml must pass the validated release branch as a job output")
end

unless dispatch_job["needs"] == "close-release-pr"
  fail_contract("trigger-patch-release must depend on successful close-release-pr completion")
end

close_step = close_job.fetch("steps").find { |step| step["name"] == "Find and close matching PR" }
dispatch_step = dispatch_job.fetch("steps").find { |step| step["name"] == "Trigger patch release" }

unless close_step&.dig("env", "GH_REPO") == "${{ github.repository }}"
  fail_contract("the PR-write job must set GH_REPO after removing checkout")
end

unless dispatch_step&.dig("env", "GH_REPO") == "${{ github.repository }}"
  fail_contract("the Actions-write job must set GH_REPO after removing checkout")
end

puts "Phase 2 permission contracts passed."
