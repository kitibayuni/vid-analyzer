#include "hdf.hpp"
#include "hdf_types.hpp"

extern "C" {
void cv_hdf_open_const_StringR(const char* HDF5Filename, Result<cv::Ptr<cv::hdf::HDF5>*>* ocvrs_return) {
	try {
		cv::Ptr<cv::hdf::HDF5> ret = cv::hdf::open(std::string(HDF5Filename));
		Ok(new cv::Ptr<cv::hdf::HDF5>(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_close(cv::hdf::HDF5* instance, ResultVoid* ocvrs_return) {
	try {
		instance->close();
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_grcreate_const_StringR(cv::hdf::HDF5* instance, const char* grlabel, ResultVoid* ocvrs_return) {
	try {
		instance->grcreate(std::string(grlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_hlexists_const_const_StringR(const cv::hdf::HDF5* instance, const char* label, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->hlexists(std::string(label));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atexists_const_const_StringR(const cv::hdf::HDF5* instance, const char* atlabel, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->atexists(std::string(atlabel));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atdelete_const_StringR(cv::hdf::HDF5* instance, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atdelete(std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atwrite_const_int_const_StringR(cv::hdf::HDF5* instance, const int value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atwrite(value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atread_intX_const_StringR(cv::hdf::HDF5* instance, int* value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atread(value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atwrite_const_double_const_StringR(cv::hdf::HDF5* instance, const double value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atwrite(value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atread_doubleX_const_StringR(cv::hdf::HDF5* instance, double* value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atread(value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atwrite_const_StringR_const_StringR(cv::hdf::HDF5* instance, const char* value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atwrite(std::string(value), std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atread_StringX_const_StringR(cv::hdf::HDF5* instance, void** value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		std::string value_out;
		instance->atread(&value_out, std::string(atlabel));
		*value = ocvrs_create_string(value_out.c_str());
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atwrite_const__InputArrayR_const_StringR(cv::hdf::HDF5* instance, const cv::_InputArray* value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atwrite(*value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_atread_const__OutputArrayR_const_StringR(cv::hdf::HDF5* instance, const cv::_OutputArray* value, const char* atlabel, ResultVoid* ocvrs_return) {
	try {
		instance->atread(*value, std::string(atlabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_int_const_int_const_StringR(const cv::hdf::HDF5* instance, const int rows, const int cols, const int type, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(rows, cols, type, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_int_const_int_const_StringR_const_int(const cv::hdf::HDF5* instance, const int rows, const int cols, const int type, const char* dslabel, const int compresslevel, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(rows, cols, type, std::string(dslabel), compresslevel);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_int_const_int_const_StringR_const_int_const_vectorLintGR(const cv::hdf::HDF5* instance, const int rows, const int cols, const int type, const char* dslabel, const int compresslevel, const std::vector<int>* dims_chunks, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(rows, cols, type, std::string(dslabel), compresslevel, *dims_chunks);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_intX_const_int_const_StringR(const cv::hdf::HDF5* instance, const int n_dims, const int* sizes, const int type, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(n_dims, sizes, type, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_intX_const_int_const_StringR_const_int(const cv::hdf::HDF5* instance, const int n_dims, const int* sizes, const int type, const char* dslabel, const int compresslevel, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(n_dims, sizes, type, std::string(dslabel), compresslevel);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_vectorLintGR_const_int_const_StringR_const_int_const_vectorLintGR(const cv::hdf::HDF5* instance, const std::vector<int>* sizes, const int type, const char* dslabel, const int compresslevel, const std::vector<int>* dims_chunks, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(*sizes, type, std::string(dslabel), compresslevel, *dims_chunks);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_vectorLintGR_const_int_const_StringR(const cv::hdf::HDF5* instance, const std::vector<int>* sizes, const int type, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(*sizes, type, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dscreate_const_const_int_const_intX_const_int_const_StringR_const_int_const_intX(const cv::hdf::HDF5* instance, const int n_dims, const int* sizes, const int type, const char* dslabel, const int compresslevel, const int* dims_chunks, ResultVoid* ocvrs_return) {
	try {
		instance->dscreate(n_dims, sizes, type, std::string(dslabel), compresslevel, dims_chunks);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsgetsize_const_const_StringR_int(const cv::hdf::HDF5* instance, const char* dslabel, int dims_flag, Result<std::vector<int>*>* ocvrs_return) {
	try {
		std::vector<int> ret = instance->dsgetsize(std::string(dslabel), dims_flag);
		Ok(new std::vector<int>(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsgetsize_const_const_StringR(const cv::hdf::HDF5* instance, const char* dslabel, Result<std::vector<int>*>* ocvrs_return) {
	try {
		std::vector<int> ret = instance->dsgetsize(std::string(dslabel));
		Ok(new std::vector<int>(ret), ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsgettype_const_const_StringR(const cv::hdf::HDF5* instance, const char* dslabel, Result<int>* ocvrs_return) {
	try {
		int ret = instance->dsgettype(std::string(dslabel));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dswrite_const_const__InputArrayR_const_StringR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dswrite(*Array, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dswrite_const_const__InputArrayR_const_StringR_const_vectorLintGR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, const std::vector<int>* dims_counts, ResultVoid* ocvrs_return) {
	try {
		instance->dswrite(*Array, std::string(dslabel), *dims_offset, *dims_counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dswrite_const_const__InputArrayR_const_StringR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, ResultVoid* ocvrs_return) {
	try {
		instance->dswrite(*Array, std::string(dslabel), *dims_offset);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsinsert_const_const__InputArrayR_const_StringR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dsinsert(*Array, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsinsert_const_const__InputArrayR_const_StringR_const_vectorLintGR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, const std::vector<int>* dims_counts, ResultVoid* ocvrs_return) {
	try {
		instance->dsinsert(*Array, std::string(dslabel), *dims_offset, *dims_counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsinsert_const_const__InputArrayR_const_StringR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_InputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, ResultVoid* ocvrs_return) {
	try {
		instance->dsinsert(*Array, std::string(dslabel), *dims_offset);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsread_const_const__OutputArrayR_const_StringR(const cv::hdf::HDF5* instance, const cv::_OutputArray* Array, const char* dslabel, ResultVoid* ocvrs_return) {
	try {
		instance->dsread(*Array, std::string(dslabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsread_const_const__OutputArrayR_const_StringR_const_vectorLintGR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_OutputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, const std::vector<int>* dims_counts, ResultVoid* ocvrs_return) {
	try {
		instance->dsread(*Array, std::string(dslabel), *dims_offset, *dims_counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_dsread_const_const__OutputArrayR_const_StringR_const_vectorLintGR(const cv::hdf::HDF5* instance, const cv::_OutputArray* Array, const char* dslabel, const std::vector<int>* dims_offset, ResultVoid* ocvrs_return) {
	try {
		instance->dsread(*Array, std::string(dslabel), *dims_offset);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpgetsize_const_const_StringR_int(const cv::hdf::HDF5* instance, const char* kplabel, int dims_flag, Result<int>* ocvrs_return) {
	try {
		int ret = instance->kpgetsize(std::string(kplabel), dims_flag);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpgetsize_const_const_StringR(const cv::hdf::HDF5* instance, const char* kplabel, Result<int>* ocvrs_return) {
	try {
		int ret = instance->kpgetsize(std::string(kplabel));
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpcreate_const_const_int_const_StringR_const_int_const_int(const cv::hdf::HDF5* instance, const int size, const char* kplabel, const int compresslevel, const int chunks, ResultVoid* ocvrs_return) {
	try {
		instance->kpcreate(size, std::string(kplabel), compresslevel, chunks);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpcreate_const_const_int_const_StringR(const cv::hdf::HDF5* instance, const int size, const char* kplabel, ResultVoid* ocvrs_return) {
	try {
		instance->kpcreate(size, std::string(kplabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpwrite_const_const_vectorLKeyPointG_const_StringR_const_int_const_int(const cv::hdf::HDF5* instance, const std::vector<cv::KeyPoint>* keypoints, const char* kplabel, const int offset, const int counts, ResultVoid* ocvrs_return) {
	try {
		instance->kpwrite(*keypoints, std::string(kplabel), offset, counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpwrite_const_const_vectorLKeyPointG_const_StringR(const cv::hdf::HDF5* instance, const std::vector<cv::KeyPoint>* keypoints, const char* kplabel, ResultVoid* ocvrs_return) {
	try {
		instance->kpwrite(*keypoints, std::string(kplabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpinsert_const_const_vectorLKeyPointG_const_StringR_const_int_const_int(const cv::hdf::HDF5* instance, const std::vector<cv::KeyPoint>* keypoints, const char* kplabel, const int offset, const int counts, ResultVoid* ocvrs_return) {
	try {
		instance->kpinsert(*keypoints, std::string(kplabel), offset, counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpinsert_const_const_vectorLKeyPointG_const_StringR(const cv::hdf::HDF5* instance, const std::vector<cv::KeyPoint>* keypoints, const char* kplabel, ResultVoid* ocvrs_return) {
	try {
		instance->kpinsert(*keypoints, std::string(kplabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpread_const_vectorLKeyPointGR_const_StringR_const_int_const_int(const cv::hdf::HDF5* instance, std::vector<cv::KeyPoint>* keypoints, const char* kplabel, const int offset, const int counts, ResultVoid* ocvrs_return) {
	try {
		instance->kpread(*keypoints, std::string(kplabel), offset, counts);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_kpread_const_vectorLKeyPointGR_const_StringR(const cv::hdf::HDF5* instance, std::vector<cv::KeyPoint>* keypoints, const char* kplabel, ResultVoid* ocvrs_return) {
	try {
		instance->kpread(*keypoints, std::string(kplabel));
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cv_hdf_HDF5_delete(cv::hdf::HDF5* instance) {
		delete instance;
}

}
