#!/usr/bin/env bash
# Render every case as a picture, store each new picture in a branch of this
# repository under the SHA-256 of its bytes, and show them all in the pull
# request's description, between two markers.
#
#   test-diagrams.sh <diagram command> <cases directory>
#
# From the environment:
#   REPO       owner/name of the repository (GITHUB_REPOSITORY)
#   PR_NUMBER  the pull request whose description shows the pictures
#   HEAD_SHA   the commit the pictures are of, named in the description
#   PUBLISH    "true" to store the pictures and edit the description; anything
#              else renders only (a pull request from a fork has a read-only
#              token)
#   GH_TOKEN   the token gh uses
#   ASSETS     the branch the pictures are stored in (default
#              assets/test-diagrams)
#
# The branch's files are named by their content, so a picture is stored once,
# and a link to it by branch never changes what it shows.

set -euo pipefail

diagram=${1:?usage: test-diagrams.sh <diagram command> <cases directory>}
cases=${2:?usage: test-diagrams.sh <diagram command> <cases directory>}
assets=${ASSETS:-assets/test-diagrams}
publish=${PUBLISH:-false}
start_marker='<!-- test-diagrams -->'
end_marker='<!-- /test-diagrams -->'
body_limit=65536

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# A name from the tree goes into file names, Markdown and HTML: take only
# names that are safe in all three.
safe() {
	[[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]
}

# Each picture is one line of the manifest: group, case, kind, sha256, ext.
manifest=$work/manifest.tsv
: >"$manifest"
mkdir -p "$work/out"

add() { # <group> <case> <kind> <rendered file>
	local sha ext
	sha=$(sha256sum "$4" | cut -d' ' -f1)
	ext=${4##*.}
	mv "$4" "$work/out/$sha.$ext"
	printf '%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$sha" "$ext" >>"$manifest"
}

# Every diagram of every type, as a still.
for dir in "$cases"/*/; do
	group=$(basename "$dir")
	case $group in invalid | diff) continue ;; esac
	if ! safe "$group"; then
		echo "::warning::skipped a directory of cases whose name is not plain: $group"
		continue
	fi
	for file in "$dir"*.diagram; do
		[ -e "$file" ] || continue
		name=$(basename "$file" .diagram)
		if ! safe "$name"; then
			echo "::warning::skipped a case whose name is not plain in $group"
			continue
		fi
		"$diagram" render "$file" -o "$work/render.png"
		add "$group" "$name" still "$work/render.png"
	done
done

# Every pair of versions, as the first becoming the second, and as a still.
for dir in "$cases"/diff/*/; do
	[ -e "$dir/before.diagram" ] && [ -e "$dir/after.diagram" ] || continue
	name=$(basename "$dir")
	if ! safe "$name"; then
		echo "::warning::skipped a diff case whose name is not plain"
		continue
	fi
	"$diagram" diff "$dir/before.diagram" "$dir/after.diagram" -o "$work/render.gif"
	add diff "$name" animation "$work/render.gif"
	"$diagram" diff "$dir/before.diagram" "$dir/after.diagram" -o "$work/render.png"
	add diff "$name" still "$work/render.png"
done

count=$(wc -l <"$manifest")
echo "rendered $count pictures"

if [ "$publish" != true ]; then
	echo "::notice::The pictures were rendered but not published: this pull request's token cannot write to the repository (a pull request from a fork)."
	exit 0
fi

: "${REPO:?}" "${PR_NUMBER:?}" "${HEAD_SHA:?}"

# The tip of the branch, or nothing when there is no branch yet. A missing
# ref makes gh print an error body, so only a commit id counts.
tip_of_branch() {
	local sha
	sha=$(gh api "repos/$REPO/git/ref/heads/$assets" --jq .object.sha 2>/dev/null || true)
	if [[ $sha =~ ^[0-9a-f]{40}$ ]]; then echo "$sha"; fi
}

# The names at the top of the tip's tree, one a line.
names_at() { # <commit>
	local tree
	tree=$(gh api "repos/$REPO/git/commits/$1" --jq .tree.sha)
	gh api "repos/$REPO/git/trees/$tree" >"$work/tree.json"
	if [ "$(jq -r .truncated "$work/tree.json")" = true ]; then
		echo "::error::the tree of $assets is too large to list whole"
		exit 1
	fi
	jq -r '.tree[].path' "$work/tree.json"
}

# Store the new pictures in one commit on the branch, and say which they were.
# If the branch moves before the commit lands, build it again on the new tip.
: >"$work/blobs.tsv" # file name, blob id: each uploaded once
cut -f4,5 "$manifest" | sort -u | tr '\t' . >"$work/wanted"
committed=false
for attempt in 1 2 3 4 5; do
	tip=$(tip_of_branch)
	if [ -n "$tip" ]; then
		names_at "$tip" | sort -u >"$work/present"
	else
		: >"$work/present"
	fi
	comm -23 "$work/wanted" "$work/present" >"$work/new"
	if [ ! -s "$work/new" ]; then
		echo "every picture is already in $assets"
		committed=true
		break
	fi

	: >"$work/entries.jsonl"
	while read -r file; do
		blob=$(awk -F'\t' -v f="$file" '$1 == f { print $2 }' "$work/blobs.tsv")
		if [ -z "$blob" ]; then
			base64 -w0 "$work/out/$file" |
				jq -Rs '{content: ., encoding: "base64"}' >"$work/blob.json"
			blob=$(gh api -X POST "repos/$REPO/git/blobs" --input "$work/blob.json" --jq .sha)
			printf '%s\t%s\n' "$file" "$blob" >>"$work/blobs.tsv"
		fi
		jq -nc --arg path "$file" --arg sha "$blob" \
			'{path: $path, mode: "100644", type: "blob", sha: $sha}' >>"$work/entries.jsonl"
	done <"$work/new"

	if [ -n "$tip" ]; then
		base_tree=$(gh api "repos/$REPO/git/commits/$tip" --jq .tree.sha)
		jq -s --arg base "$base_tree" '{base_tree: $base, tree: .}' "$work/entries.jsonl" >"$work/tree-req.json"
	else
		jq -s '{tree: .}' "$work/entries.jsonl" >"$work/tree-req.json"
	fi
	tree=$(gh api -X POST "repos/$REPO/git/trees" --input "$work/tree-req.json" --jq .sha)

	added=$(wc -l <"$work/new")
	jq -n --arg tree "$tree" --arg tip "$tip" \
		--arg message "Add $added test diagrams from pull request #$PR_NUMBER at ${HEAD_SHA:0:12}" \
		'{message: $message, tree: $tree,
		  parents: (if $tip == "" then [] else [$tip] end),
		  author: {name: "github-actions[bot]",
		           email: "41898282+github-actions[bot]@users.noreply.github.com"}}' \
		>"$work/commit-req.json"
	commit=$(gh api -X POST "repos/$REPO/git/commits" --input "$work/commit-req.json" --jq .sha)

	if [ -n "$tip" ]; then
		moved=$(gh api -X PATCH "repos/$REPO/git/refs/heads/$assets" \
			-f sha="$commit" -F force=false --jq .object.sha 2>/dev/null || true)
	else
		moved=$(gh api -X POST "repos/$REPO/git/refs" \
			-f ref="refs/heads/$assets" -f sha="$commit" --jq .object.sha 2>/dev/null || true)
	fi
	if [ "$moved" = "$commit" ]; then
		echo "stored $added new pictures in $assets as $commit"
		committed=true
		break
	fi
	echo "$assets moved while storing (attempt $attempt); building on its new tip"
	sleep $((attempt * 2))
done
if [ "$committed" != true ]; then
	echo "::error::could not update $assets"
	exit 1
fi

# The gallery: a heading, then each group of cases folded, each picture with
# its case's name, and the ones this run added marked new.
raw="https://github.com/$REPO/raw/$assets"
gallery=$work/gallery.md
{
	echo "$start_marker"
	echo "### Test diagrams"
	echo
	fresh=$(wc -l <"$work/new")
	if [ "$fresh" -gt 0 ]; then
		echo "Every case in \`crates/pipeline/cases/\`, rendered at ${HEAD_SHA:0:12}: $count pictures, $fresh of them new on \`$assets\` in this run, marked **new**."
	else
		echo "Every case in \`crates/pipeline/cases/\`, rendered at ${HEAD_SHA:0:12}: $count pictures, none of them new on \`$assets\`."
	fi
	echo
	cut -f1 "$manifest" | awk '!seen[$0]++' | while read -r group; do
		total=$(awk -F'\t' -v g="$group" '$1 == g' "$manifest" | wc -l)
		echo "<details><summary>$group ($total)</summary>"
		echo
		awk -F'\t' -v g="$group" '$1 == g' "$manifest" |
			while IFS=$'\t' read -r _ name kind sha ext; do
				file=$sha.$ext
				label=$name
				[ "$kind" = animation ] && label="$name, animated"
				mark=""
				grep -qxF "$file" "$work/new" && mark=" <b>new</b>"
				printf '<p><a href="%s/%s"><img src="%s/%s" alt="%s" width="640"></a><br><sub>%s%s</sub></p>\n' \
					"$raw" "$file" "$raw" "$file" "$label" "$label" "$mark"
			done
		echo
		echo "</details>"
	done
	echo "$end_marker"
} >"$gallery"

# Put the gallery between the markers, or after the author's text when there
# are none, and leave the rest of the description as it is.
gh api "repos/$REPO/pulls/$PR_NUMBER" >"$work/pr.json"
jq -j --rawfile gallery "$gallery" --arg start "$start_marker" --arg end "$end_marker" '
	(.body // "") as $body
	| ($gallery | rtrimstr("\n")) as $gallery
	| if ($body | contains($start)) and (($body | split($start)[1:] | join($start)) | contains($end)) then
		($body | split($start)) as $parts
		| ($parts[1:] | join($start)) as $rest
		| $parts[0] + $gallery + ($rest | split($end)[1:] | join($end))
	  elif ($body | test("^\\s*$")) then $gallery
	  else ($body | sub("\\s+$"; "")) + "\n\n" + $gallery
	  end' "$work/pr.json" >"$work/body.md"

if [ "$(jq -Rs 'length' "$work/body.md")" -ge "$body_limit" ]; then
	echo "::warning::the description with the gallery would pass GitHub's limit of $body_limit characters; left as it was"
	exit 0
fi
if [ "$(jq -r '.body // ""' "$work/pr.json")" = "$(cat "$work/body.md")" ]; then
	echo "the description already shows these pictures"
	exit 0
fi
jq -Rs '{body: .}' "$work/body.md" >"$work/body.json"
gh api -X PATCH "repos/$REPO/pulls/$PR_NUMBER" --input "$work/body.json" --silent
echo "showed $count pictures in the description of #$PR_NUMBER"
