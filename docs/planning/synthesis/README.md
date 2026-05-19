# Synthesis

Four documents that pull cross-tool conclusions out of the per-tool research:

- [patterns.md](patterns.md) — what every serious sprite/animation tool has converged on, and what's still fragmented
- [gaps.md](gaps.md) — workflows that hurt across every tool and have no current solution
- [ai-opportunity.md](ai-opportunity.md) — opinionated map of where AI has real leverage versus where it would regress UX
- [prior-art.md](prior-art.md) — homogeneous digest of the seven deep prior-art dossiers in [`../research/`](../research/): recurring patterns across them, open conflicts with the locked plan, port roadmap, attribution discipline

Read them in order. `patterns.md` defines the floor any new tool has to clear. `gaps.md` defines the surface where new value is possible. `ai-opportunity.md` separates the AI-fixable subset of those gaps from the rest, and from the bucket of bad-AI-features that look tempting but ship worse products. `prior-art.md` then drops a layer deeper: where the first three are the broad May 3 tool survey, the prior-art digest is the consolidated "what to do" extracted from the dossiers added in May 14–19 (PRs #213–#219). When the dossiers and the broader synthesis disagree, the dossiers are the more recent evidence; surface the conflict in `prior-art.md` § "Open decisions" rather than silently overriding `patterns.md`.
