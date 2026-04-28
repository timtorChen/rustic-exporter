terraform {
  required_version = "~> 1.14.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.13.0"
    }
    github = {
      source  = "integrations/github"
      version = "~> 6.12.0"
    }
  }
}

provider "github" {}

locals {
  github_repository = "rustic-exporter"
  github_labels = {
    "feat" = {
      color       = "#a8d8b9" # 白綠
      description = "New feature"
    },
    "bug" = {
      color       = "#f4a7b9" # 一斥染
      description = "Bug or Unexpected behavior"
    },
    "refactor" = {
      color       = "#d4c5f9"
      description = "Code improvement without behavior change"
    },
    "chore" = {
      color       = "#bdc0ba" # 白鼠
      description = "CI, version bumps or boring things"
    },
    "question" = {
      color       = "#DAC9A6" # 鳥之子
      description = "Quesion and discussion"
    }
  }
}

# !NOTICE
# github_issue_label id is stored as "repo_name:label_name"
resource "github_issue_label" "main" {
  repository  = local.github_repository
  for_each    = local.github_labels
  name        = each.key
  color       = replace(each.value.color, "#", "")
  description = try(each.value.description, null)
}
