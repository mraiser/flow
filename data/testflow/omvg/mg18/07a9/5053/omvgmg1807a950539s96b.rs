let mut ax = DataArray::new();
ax.push_property(a);
ax.push_property(b);
Data::DArray(ax.data_ref)