#include <inputosm/inputosm.hpp>
#include <bit>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <mutex>
#include <string_view>
#include <vector>

struct digest_t {
    uint64_t value;
    digest_t(uint64_t kind, int64_t id) : value(kind) { number(uint64_t(id)); }
    void number(uint64_t n) { value = std::rotl(value, 7) + n; }
    void bytes(std::string_view bytes) {
        number(bytes.size());
        for (unsigned char c : bytes) number(c);
    }
    void tags(std::span<const input_osm::tag_t> tags) {
        for (const auto& t : tags) { bytes(t.key); bytes(t.value); }
    }
};
struct alignas(64) counter_t {
    std::mutex mutex;
    uint64_t nodes = 0, ways = 0, relations = 0, checksum = 0;
};

int main(int argc, char** argv) {
    if (argc != 4 || std::string_view(argv[2]) != "entities") return 2;
    const auto threads = std::strtoul(argv[3], nullptr, 10);
    if (threads < 1 || threads > 256) return 2;
    input_osm::set_thread_count(threads);
    input_osm::set_log_level(input_osm::LOG_LEVEL_DISABLED);
    std::vector<counter_t> counters(threads);
    auto start = std::chrono::steady_clock::now();
    bool ok = input_osm::input_file(argv[1], false,
        [&](std::span<const input_osm::node_t> nodes) {
            auto& t = counters[input_osm::thread_index];
            std::lock_guard lock(t.mutex);
            t.nodes += nodes.size();
            for (const auto& n : nodes) {
                digest_t h(1, n.id);
                h.number(uint64_t(n.raw_latitude));
                h.number(uint64_t(n.raw_longitude));
                h.tags(n.tags);
                t.checksum += h.value;
            }
            return true;
        },
        [&](std::span<const input_osm::way_t> ways) {
            auto& t = counters[input_osm::thread_index];
            std::lock_guard lock(t.mutex);
            t.ways += ways.size();
            for (const auto& w : ways) {
                digest_t h(2, w.id);
                h.tags(w.tags);
                h.number(w.node_refs.size());
                for (auto id : w.node_refs) h.number(uint64_t(id));
                t.checksum += h.value;
            }
            return true;
        },
        [&](std::span<const input_osm::relation_t> relations) {
            auto& t = counters[input_osm::thread_index];
            std::lock_guard lock(t.mutex);
            t.relations += relations.size();
            for (const auto& r : relations) {
                digest_t h(3, r.id);
                h.tags(r.tags);
                h.number(r.members.size());
                for (const auto& m : r.members) {
                    h.number(uint64_t(m.id));
                    h.number(m.type);
                    h.bytes(m.role);
                }
                t.checksum += h.value;
            }
            return true;
        });
    double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
    if (!ok) return 1;
    uint64_t nodes = 0, ways = 0, relations = 0, checksum = 0;
    for (const auto& t : counters) {
        nodes += t.nodes; ways += t.ways; relations += t.relations; checksum += t.checksum;
    }
    std::puts("mode,threads,seconds,nodes,ways,relations,checksum,node_bytes,way_bytes,relation_bytes");
    std::printf("entities,%lu,%.9f,%lu,%lu,%lu,%lu,%zu,%zu,%zu\n", threads, seconds,
        nodes, ways, relations, checksum, sizeof(input_osm::node_t), sizeof(input_osm::way_t), sizeof(input_osm::relation_t));
}
