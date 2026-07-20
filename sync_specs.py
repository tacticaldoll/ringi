import os
import glob
import re

delta_dir = "openspec/changes/reframe-ringi-deliberation/specs"
main_dir = "openspec/specs"

delta_specs = glob.glob(os.path.join(delta_dir, "**/*.md"), recursive=True)

for delta_spec_path in delta_specs:
    rel_path = os.path.relpath(delta_spec_path, delta_dir)
    main_spec_path = os.path.join(main_dir, rel_path)
    
    with open(delta_spec_path, "r") as f:
        delta_content = f.read()

    # Parse sections
    added_matches = re.search(r'## ADDED Requirements(.*?)(\n##|$)', delta_content, re.DOTALL)
    modified_matches = re.search(r'## MODIFIED Requirements(.*?)(\n##|$)', delta_content, re.DOTALL)
    removed_matches = re.search(r'## REMOVED Requirements(.*?)(\n##|$)', delta_content, re.DOTALL)
    
    if os.path.exists(main_spec_path):
        with open(main_spec_path, "r") as f:
            main_content = f.read()
    else:
        # Create directory if it doesn't exist
        os.makedirs(os.path.dirname(main_spec_path), exist_ok=True)
        # Assuming we can infer the header
        spec_name = os.path.basename(os.path.dirname(main_spec_path))
        main_content = f"# {spec_name} Specification\n\n## Purpose\n\n(Generated)\n\n## Requirements\n"

    new_content = main_content

    if modified_matches:
        modified_reqs = modified_matches.group(1).strip()
        # Each requirement starts with `### Requirement: ...`
        req_blocks = re.split(r'\n(?=### Requirement:)', '\n' + modified_reqs)[1:]
        for req_block in req_blocks:
            req_name_match = re.match(r'### Requirement: (.*?)\n', req_block)
            if req_name_match:
                req_name = req_name_match.group(1).strip()
                # Find it in main_content and replace
                pattern = r'### Requirement: ' + re.escape(req_name) + r'.*?(?=\n### Requirement:|\Z)'
                if re.search(pattern, new_content, re.DOTALL):
                    new_content = re.sub(pattern, req_block.strip() + "\n", new_content, flags=re.DOTALL)
                else:
                    new_content += "\n" + req_block.strip() + "\n"

    if added_matches:
        added_reqs = added_matches.group(1).strip()
        new_content += "\n" + added_reqs + "\n"

    if removed_matches:
        removed_reqs = removed_matches.group(1).strip()
        req_blocks = re.split(r'\n(?=### Requirement:)', '\n' + removed_reqs)[1:]
        for req_block in req_blocks:
            req_name_match = re.match(r'### Requirement: (.*?)\n', req_block)
            if req_name_match:
                req_name = req_name_match.group(1).strip()
                pattern = r'### Requirement: ' + re.escape(req_name) + r'.*?(?=\n### Requirement:|\Z)'
                new_content = re.sub(pattern, "", new_content, flags=re.DOTALL)

    # Clean up empty lines
    new_content = re.sub(r'\n{3,}', '\n\n', new_content)
    
    with open(main_spec_path, "w") as f:
        f.write(new_content)
    
    print(f"Synced {main_spec_path}")
