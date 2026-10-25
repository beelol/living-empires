"""Verify a private Studio delivery without extracting or activating anything."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

try:
    from .bridge import (Journal, canonical, fingerprint, is_hash, require,
                         validate_evidence_metadata)
except ImportError:
    from bridge import (Journal, canonical, fingerprint, is_hash, require,
                        validate_evidence_metadata)


MAX_ARCHIVE = 128 * 1024 * 1024
MAX_MANIFEST = 16 * 1024 * 1024
MAX_IMAGE = 20 * 1024 * 1024


def verify_archive(path, expected_sha256):
    """Expected outer hash must arrive over a trusted channel from the producer.

    This establishes archive integrity only. It is not asset approval, a visual
    review, a signature from the simulation, or evidence of live game outcomes.
    """
    path = Path(path)
    require(is_hash(expected_sha256), 'expected producer archive SHA256 required')
    require(path.stat().st_size <= MAX_ARCHIVE, 'archive exceeds receiver limit')
    try:
        with path.open('rb') as source:
            digest = hashlib.sha256()
            total = 0
            for block in iter(lambda: source.read(1024 * 1024), b''):
                total += len(block)
                require(total <= MAX_ARCHIVE, 'archive exceeds receiver limit')
                digest.update(block)
            require(digest.hexdigest() == expected_sha256, 'archive SHA256 mismatch')
            source.seek(0)
            with zipfile.ZipFile(source) as archive:
                infos = archive.infolist()
                names = [i.filename for i in infos]
                require(len(names) == len(set(names)), 'duplicate archive member')
                require(sum(i.file_size for i in infos) <= MAX_ARCHIVE,
                        'expanded archive exceeds receiver limit')
                require('manifest.json' in names, 'missing archive manifest')
                require(archive.getinfo('manifest.json').file_size <= MAX_MANIFEST,
                        'manifest exceeds receiver limit')
                manifest = json.loads(archive.read('manifest.json'))
                require(set(manifest) == {'version', 'audience', 'records', 'evidence'} and
                        type(manifest['version']) is int and manifest['version'] == 1 and
                        manifest['audience'] == 'trusted-operator-only',
                        'unsupported Studio archive schema/audience')
                require(isinstance(manifest['records'], list) and
                        isinstance(manifest['evidence'], dict), 'invalid manifest collections')
                evidence = manifest['evidence']
                require(all(is_hash(h) for h in evidence), 'invalid evidence ID')
                require(set(names) == {'manifest.json'} | {'evidence/' + h for h in evidence},
                        'missing or unexpected archive member')
                for image_hash, metadata in evidence.items():
                    validate_evidence_metadata(metadata)
                    info = archive.getinfo('evidence/' + image_hash)
                    require(0 < info.file_size <= MAX_IMAGE, 'image exceeds receiver limit')
                    data = archive.read(info)
                    require(fingerprint(dict(bytes_hash=hashlib.sha256(data).hexdigest(),
                                             metadata=metadata)) == image_hash,
                            'evidence SHA256 mismatch')
                previous_hash = ''
                previous_state = {}
                previous_event = None
                used_images = set()
                source_keys = set()
                for sequence, record in enumerate(manifest['records'], start=1):
                    require(isinstance(record, dict) and set(record) == {
                        'version', 'seq', 'parent', 'event', 'state_hash', 'hash', 'diff'},
                        'invalid history envelope')
                    core = {k: v for k, v in record.items() if k not in ('hash', 'diff')}
                    require(type(record['version']) is int and record['version'] == 1 and
                            type(record['seq']) is int and record['seq'] == sequence and
                            record['parent'] == previous_hash and
                            fingerprint(core) == record['hash'], 'history chain mismatch')
                    event = record['event']
                    Journal._validate(event)
                    key = (event['source'], event['key'])
                    require(key not in source_keys, 'duplicate source action')
                    source_keys.add(key)
                    state = event['state']
                    require(fingerprint(state) == record['state_hash'], 'state SHA256 mismatch')
                    expected_diff = dict(set={k: v for k, v in state.items() if k not in
                        previous_state or canonical(previous_state[k]) != canonical(v)},
                        remove=sorted(set(previous_state) - set(state)))
                    require(canonical(expected_diff) == canonical(record['diff']),
                            'derived diff does not match history')
                    if previous_event:
                        Journal._check_successor(previous_event, event)
                    for image_hash in event['evidence']:
                        require(image_hash in evidence, 'event image missing')
                        meta = evidence[image_hash]
                        require(meta['assets'] == event['identity']['assets'] and
                                meta['tick'] == event['tick'] and meta['at'] == event['at'],
                                'image time/content does not match event')
                        used_images.add(image_hash)
                    previous_hash, previous_state, previous_event = record['hash'], state, event
                require(used_images == set(evidence), 'unreferenced image in delivery')
                return dict(version=1, sha256=expected_sha256, head=previous_hash,
                            events=len(manifest['records']), images=len(evidence))
    except (KeyError, TypeError, json.JSONDecodeError, zipfile.BadZipFile,
            RecursionError, OverflowError) as error:
        raise ValueError('invalid Studio archive') from error


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--sha256', required=True)
    args = parser.parse_args()
    print(canonical(verify_archive(args.archive, args.sha256)).decode())
