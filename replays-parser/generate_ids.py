import os
import sys
import json
import xml.etree.ElementTree as ET
import re

def find_file(base_path, filename, search_subdirs=None):
    """
    Search for a file in base_path and specified subdirectories.
    """
    base_path = os.path.normpath(base_path)
    candidates = [
        os.path.join(base_path, filename),
        os.path.join(base_path, 'scripts', filename),
        os.path.join(base_path, 'res', 'scripts', filename),
        os.path.join(base_path, 'source', 'res', 'scripts', filename)
    ]
    
    if search_subdirs:
        for sub in search_subdirs:
            candidates.append(os.path.join(base_path, sub, filename))
            candidates.append(os.path.join(base_path, 'scripts', sub, filename))
            candidates.append(os.path.join(base_path, 'res', 'scripts', sub, filename))
            candidates.append(os.path.join(base_path, 'source', 'res', 'scripts', sub, filename))

    for p in candidates:
        # print(f"DEBUG: Checking {p}") 
        if os.path.exists(p):
            print(f"DEBUG: Found {filename} at {p}")
            return p
    print(f"DEBUG: Could not find {filename} in {base_path}")
    return None

def detect_game_version(base_path):
    """
    Attempts to detect the game version and region from the game client files.
    Returns a string like 'wot_eu_1_25_1_0' or 'unknown_version'.
    """
    version = "unknown"
    region = "unknown"
    
    # 1. Try to find version.xml
    # It often looks like <version>v.1.25.1.0 #1234</version>
    v_path = find_file(base_path, 'version.xml')
    if v_path:
        try:
            tree = ET.parse(v_path)
            root = tree.getroot()
            if root.tag == 'version' and root.text:
                version = root.text.strip()
        except:
            pass
            
    # Cleaning version string
    # Remove 'v.' prefix if present
    if version.startswith('v.'):
        version = version[2:]
        
    # 2. Try to guess region from path (common user convention)
    base_name = os.path.basename(os.path.normpath(base_path)).lower()
    if 'eu' in base_name: region = 'eu'
    elif 'na' in base_name: region = 'na'
    elif 'ru' in base_name: region = 'ru'
    elif 'asia' in base_name: region = 'asia'
    elif 'cn' in base_name: region = 'cn'
    elif 'ct' in base_name: region = 'ct' # Common Test
    
    # Construct final ID string
    # sanitize version string to be file-friendly
    safe_ver = re.sub(r'[^a-zA-Z0-9]', '_', version)
    safe_ver = re.sub(r'_+', '_', safe_ver).strip('_')
    
    if region != 'unknown':
        return f"wot_{region}_v{safe_ver}"
    else:
        return f"wot_v{safe_ver}"


def parse_entities_xml(base_path):
    """
    Parses entities.xml to get Entity Type IDs.
    Returns: {id: name}
    """
    path = find_file(base_path, 'entities.xml')
    if not path:
        print(f"Error: Could not find entities.xml in {base_path}")
        sys.exit(1)
        
    print(f"Parsing entities from: {path}")
    tree = ET.parse(path)
    root = tree.getroot()
    
    entities = {}
    # Find ClientServerEntities tag
    if root.tag == 'ClientServerEntities':
        cs_entities = root
    else:
        cs_entities = root.find('ClientServerEntities')

    if cs_entities is not None:
        for idx, child in enumerate(cs_entities):
            # The tag name represents the Entity Name (e.g. <Avatar/>)
            # The index is the Entity ID.
            print(f"DEBUG: Found Entity {child.tag} ID {idx}")
            entities[idx] = child.tag
    else:
        print("DEBUG: ClientServerEntities tag not found in entities.xml")
            
    return entities

def get_def_path(base_path, entity_name):
    """
    Finds the .def file for an entity or interface.
    """
    # Interfaces are usually in 'entity_defs/interfaces'
    # Regular entities in 'entity_defs'
    # We search both.
    path = find_file(base_path, f"{entity_name}.def", 
                    search_subdirs=['entity_defs', 'entity_defs/interfaces'])
    return path

def collect_methods(base_path, def_path, visited=None):
    """
    Recursively collects methods and properties from a .def file.
    visited: list of file paths to detect cycles.
    Returns: {'ClientMethods': [], 'Properties': [], ...}
    """
    if visited is None:
        visited = []

    if def_path in visited:
        print(f"  [Cycle Detected] {def_path} already visited.")
        return {'ClientMethods': [], 'Properties': [], 'CellMethods': [], 'BaseMethods': []}

    visited.append(def_path)
    
    results = {
        'ClientMethods': {},
        'Properties': {},
        'CellMethods': {},
        'BaseMethods': {}
    }

    try:
        tree = ET.parse(def_path)
        root = tree.getroot()

        # 1. Recursively process Interfaces (<Implements>)
        implements = root.find('Implements')
        if implements is not None:
            for interface in implements.findall('Interface'):
                interface_name = interface.text.strip()
                interface_def = get_def_path(base_path, interface_name)
                
                if interface_def:
                    # RECURSION
                    inherited = collect_methods(base_path, interface_def, visited)
                    
                    # Merge inherited methods (append preserves order)
                    for key in results:
                        results[key].update(inherited[key])
                else:
                    print(f"  [Warning] Interface {interface_name} not found.")

        # 2. Process Local Definitions
        for section in results.keys():
            xml_section = root.find(section)
            if xml_section is not None:
                for child in xml_section:
                    # method/property name is the tag name
                    name = child.tag
                    # Collect metadata for filtering
                    is_excluded = False
                    arg_count = len(child.findall('Arg'))
                    
                    # Log logic for debugging Vehicle ClientMethods
                    if section == 'ClientMethods' and 'Vehicle.def' in def_path:
                         print(f"DEBUG: Processing {name}. Args: {arg_count}. DetailDistance: {child.find('DetailDistance') is not None}")

                    # Exclusion Rules Hypothesis:
                    # 1. Exclude 0-argument methods (signals?) - Verified with onVehiclePickup rejection?
                    # 2. Exclude methods with DetailDistance (LOD-based)
                    
                    if child.find('DetailDistance') is not None:
                        is_excluded = True
                    if arg_count == 0:
                        is_excluded = True
                        
                    if not is_excluded:
                        # Extract details based on section
                        if section == 'Properties':
                            # <Type>STRING</Type>
                            type_tag = child.find('Type')
                            p_type = type_tag.text.strip() if (type_tag is not None and type_tag.text) else "UNKNOWN"
                            results[section][name] = {'type': p_type}
                        else:
                            # Methods: <Arg>INT32</Arg>...
                            args = [arg.text.strip() if arg.text else '' for arg in child.findall('Arg')]
                            results[section][name] = {'args': args}

    except Exception as e:
        print(f"  [Error] Failed to parse {def_path}: {e}")
    finally:
        visited.pop()
        
    return results

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 generate_ids.py <path_to_game_scripts>")
        sys.exit(1)
        
    print(f"DEBUG: Game Path: {sys.argv[1]}")
    game_path = sys.argv[1]
    
    # 0. Detect Version
    version_tag = detect_game_version(game_path)
    print(f"DEBUG: Detected Version Tag: {version_tag}")

    try:
        # 1. Parse Entity Types
        entity_types = parse_entities_xml(game_path)
        print(f"DEBUG: Found {len(entity_types)} entity types.")
        
        if not entity_types:
            print("ERROR: No entities found in entities.xml. Check the file content.")
            sys.exit(1)
        
        full_data = {}
        
        # 2. Load Packet Types from message_codes directory
        # Strategy:
        # 1. Determine "game code" (e.g. wot_eu) from the path or similar logic.
        #    For now, we can try to guess or just default to 'wot_eu' if not clear.
        #    But since we have region detection in detect_game_version, let's use that.
        
        region_code = "wot_eu" # Default fallback
        if "ru" in version_tag: region_code = "wot_ru"
        elif "na" in version_tag: region_code = "wot_na"
        elif "asia" in version_tag: region_code = "wot_asia"
        elif "wot_cn" in version_tag: region_code = "wot_cn"
        
        # Base directory for message codes
        msg_base = os.path.join(os.path.dirname(os.path.abspath(__file__)), "message_codes", region_code)
        
        packet_types = {}
        
        # Load _default.json
        default_path = os.path.join(msg_base, "_default.json")
        if os.path.exists(default_path):
             print(f"DEBUG: Loading default packet definitions from {default_path}")
             try:
                 with open(default_path, 'r') as f:
                     manual_data = json.load(f)
                     packet_types = manual_data.get("packetTypes", {})
             except Exception as e:
                 print(f"WARNING: Failed to load {default_path}: {e}")
        else:
             print(f"WARNING: No default packet definitions found at {default_path}")

        # Load version override if exists (e.g. 1.25.0.json)
        # version_tag is like "wot_eu_v1_25_1_0"
        # We need extracting "1.25.1.0" or similar
        # Let's try to match the version part.
        
        # Note: Implementation Plan said "Load _default.json first, then {version}.json".
        # We need to correctly identify the version file name. 
        # For now, let's assume we might find it by the safe version string.
        # But commonly overrides might be rare.
        
        # (Extension point for future: load specific version overrides)

        # 2. Process each Entity
        for ent_id, ent_name in entity_types.items():
            print(f"[{ent_id}] Processing {ent_name}...")
            
            def_path = get_def_path(game_path, ent_name)
            if not def_path:
                print(f"  [Error] No .def file found for {ent_name}")
                continue
                
            # Collect all methods (recursive)
            data = collect_methods(game_path, def_path)
            
            # Sort methods alphabetically for correct indexing
            # We need to sort the keys of the dicts
            
            # Helper to create indexed dictionary
            def to_indexed_dict(data_dict):
                sorted_keys = sorted(data_dict.keys())
                return { 
                    i: { "name": k, **data_dict[k] } 
                    for i, k in enumerate(sorted_keys) 
                }

            # Structure the output
            entity_json = {
                "id": ent_id,
                "name": ent_name,
                "clientMethods": to_indexed_dict(data['ClientMethods']),
                "properties": to_indexed_dict(data['Properties']),
                "cellMethods": to_indexed_dict(data['CellMethods']),
                "baseMethods": to_indexed_dict(data['BaseMethods'])
            }
            
            full_data[str(ent_id)] = entity_json
            
        # 3. Write JSON Output
        final_json = {
            "packetTypes": packet_types,
            "entities": full_data
        }
        
        output_path = os.path.abspath(f"ids_{version_tag}.json")
        with open(output_path, "w") as f:
            json.dump(final_json, f, indent=2)
            
        print(f"Success! Generated {output_path}")

        # 4. Write Joined XML (User Request)
        xml_path = os.path.abspath(f"joined_entities_{version_tag}.xml")
        root = ET.Element("Entities")
        
        # Sort by ID for stability
        sorted_ids = sorted(full_data.keys(), key=lambda x: int(x))
        
        for ent_id in sorted_ids:
            ent_data = full_data[ent_id]
            ent_node = ET.SubElement(root, "Entity", id=str(ent_id), name=ent_data['name'])
            
            # Client Methods
            cm_node = ET.SubElement(ent_node, "ClientMethods")
            for m_id, val in ent_data['clientMethods'].items():
                m_node = ET.SubElement(cm_node, "Method", id=str(m_id), name=val['name'])
                # Add args as children
                for arg_type in val.get('args', []):
                    ET.SubElement(m_node, "Arg").text = arg_type
                
            # Properties
            prop_node = ET.SubElement(ent_node, "Properties")
            for p_id, val in ent_data['properties'].items():
                ET.SubElement(prop_node, "Property", id=str(p_id), name=val['name'], type=val.get('type', ''))
                
            # Cell Methods
            cell_node = ET.SubElement(ent_node, "CellMethods")
            for m_id, val in ent_data['cellMethods'].items():
                m_node = ET.SubElement(cell_node, "Method", id=str(m_id), name=val['name'])
                for arg_type in val.get('args', []):
                    ET.SubElement(m_node, "Arg").text = arg_type
                
            # Base Methods
            base_node = ET.SubElement(ent_node, "BaseMethods")
            for m_id, val in ent_data['baseMethods'].items():
                m_node = ET.SubElement(base_node, "Method", id=str(m_id), name=val['name'])
                for arg_type in val.get('args', []):
                    ET.SubElement(m_node, "Arg").text = arg_type

        tree = ET.ElementTree(root)
        if hasattr(ET, 'indent'):
            ET.indent(tree, space="  ", level=0)
            
        tree.write(xml_path, encoding="utf-8", xml_declaration=True)
        print(f"Success! Generated {xml_path}")

        # 5. Write Hex Map (User Request)
        map_path = os.path.abspath(f"message_map_{version_tag}.txt")
        with open(map_path, "w") as f:
            f.write("=== World of Tanks Entity & Message Map ===\n\n")
            
            f.write("--- Network Packet Types (Engine Level) ---\n")
            f.write("(From wotreplay-parser reference)\n")
            for pid, pdata in packet_types.items():
                if isinstance(pdata, str):
                     # Legacy support or simple string
                    f.write(f"{pid}: {pdata}\n")
                elif isinstance(pdata, dict):
                    # New format: {"id": "NAME", "subtypes": ...}
                    name = pdata.get('id', 'UNKNOWN')
                    f.write(f"{pid}: {name}\n")
                    if 'subtypes' in pdata:
                        for subid, subname in pdata['subtypes'].items():
                            f.write(f"  Subtype {subid}: {subname}\n")
            f.write("\n--- Entity Method IDs (Script Level) ---\n\n")

            for ent_id in sorted_ids:
                ent = full_data[ent_id]
                f.write(f"Entity ID: {ent['id']} ({ent['name']})\n")
                
                # Helper to write section
                def write_section(title, data):
                    if not data: return
                    f.write(f"  {title}:\n")
                    # Sort by integer ID
                    for m_id in sorted(data.keys(), key=lambda x: int(x)):
                        val = data[m_id]
                        name = val['name']
                        details = ""
                        if 'args' in val:
                            details = f"({', '.join(val['args'])})"
                        elif 'type' in val:
                            details = f": {val['type']}"
                        
                        hex_id = f"0x{int(m_id):02X}"
                        # Ensure nice formatting
                        f.write(f"    {hex_id} ({m_id}): {name}{details}\n")
                
                write_section("Client Methods", ent['clientMethods'])
                write_section("Properties", ent['properties'])
                write_section("Cell Methods", ent['cellMethods'])
                write_section("Base Methods", ent['baseMethods'])
                f.write("\n")
                
        print(f"Success! Generated {map_path}")
    except Exception as e:
        import traceback
        print("CRITICAL ERROR: Script crashed!")
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
