.PHONY: all clean validate-spv validate-msl
.SECONDARY: boids.metal quad.metal
SNAPSHOTS=tests/snapshots

all:

clean:
	rm *.metal *.air *.metallib *.vert *.frag *.comp *.spv

%.metal: $(SNAPSHOTS)/in/%.wgsl $(wildcard src/*.rs src/**/*.rs examples/*.rs)
	cargo run --example convert --features wgsl-in,msl-out -- $< $@

%.air: %.metal
	xcrun -sdk macosx metal -c $< -mmacosx-version-min=10.11

%.metallib: %.air
	xcrun -sdk macosx metallib $< -o $@

%.spv: test-data/%.wgsl $(wildcard src/*.rs src/**/*.rs examples/*.rs)
	cargo run --example convert --features wgsl-in,spv-out -- $< $@
	spirv-val $@

%.vert %.frag %.comp: test-data/%.wgsl $(wildcard src/*.rs src/**/*.rs examples/*.rs)
	cargo run --example convert --features wgsl-in,glsl-out -- $< $@
	glslangValidator $@

validate-spv: $(SNAPSHOTS)/*.spvasm.snap
	@for file in $^ ; do \
		echo "Validating" $${file#"$(SNAPSHOTS)/snapshots__"};	\
		tail -n +5 $${file} | spirv-as --target-env vulkan1.0 -o - | spirv-val; \
	done

validate-msl: $(SNAPSHOTS)/*.msl.snap
	@for file in $^ ; do \
		echo "Validating" $${file#"$(SNAPSHOTS)/snapshots__"};	\
		tail -n +5 $${file} | xcrun -sdk macosx metal -mmacosx-version-min=10.11 -x metal - -o /dev/null; \
	done

validate-glsl: $(SNAPSHOTS)/*.glsl.snap
	@for file in $(SNAPSHOTS)/*-Vertex.glsl.snap ; do \
		echo "Validating" $${file#"$(SNAPSHOTS)/snapshots__"};\
		tail -n +5 $${file} | glslangValidator --stdin -S vert; \
	done
	@for file in $(SNAPSHOTS)/*-Fragment.glsl.snap ; do \
		echo "Validating" $${file#"$(SNAPSHOTS)/snapshots__"};\
		tail -n +5 $${file} | glslangValidator --stdin -S frag; \
	done
	@for file in $(SNAPSHOTS)/*-Compute.glsl.snap ; do \
		echo "Validating" $${file#"$(SNAPSHOTS)/snapshots__"};\
		tail -n +5 $${file} | glslangValidator --stdin -S comp; \
	done
